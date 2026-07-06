package workflow

import (
	"context"
	"encoding/json"
	"fmt"
	"log"
	"strings"
	"sync"

	"google.golang.org/genai"
)

const (
	// DefaultModel は選定・抽出・リサーチ・最終回答に使う既定モデル。
	DefaultModel = "gemini-2.5-flash"
	// DefaultLiteModel は未訪問判定のような軽い分類に使う既定モデル。
	DefaultLiteModel = "gemini-2.5-flash-lite"

	maxSelectedWorlds     = 10
	enrichConcurrency     = 3
	maxNewWorldCandidates = 10
)

// Run は Dify DSL (WorldRec.yml) のワークフローを Gemini Developer API 上で実行する。
//
// 流れ:
//  1. 訪問履歴候補からユーザー希望に合う上位10件を構造化出力で選定 (LLM)
//  2. 各候補を Google Search Grounding で検索し、概要・推奨人数を抽出 (イテレーション + LLM 4)
//  3. 推薦モードを visited_only / unvisited_only / mixed の3択で判定 (LLM 2)
//  4. visited_only → 訪問済み候補のみで回答 (LLM 3)
//     unvisited_only / mixed → Grounding で未訪問候補をリサーチし最終推薦文を作成 (LLM 5)
func Run(ctx context.Context, request *Request) (*Response, error) {
	if strings.TrimSpace(request.APIKey) == "" {
		return nil, fmt.Errorf("api_key is required")
	}
	if strings.TrimSpace(request.Query) == "" {
		return nil, fmt.Errorf("query is required")
	}

	client, err := genai.NewClient(ctx, &genai.ClientConfig{
		APIKey:  request.APIKey,
		Backend: genai.BackendGeminiAPI,
	})
	if err != nil {
		return nil, fmt.Errorf("gemini client init failed: %w", err)
	}

	model := request.Model
	if strings.TrimSpace(model) == "" {
		model = DefaultModel
	}
	liteModel := request.LiteModel
	if strings.TrimSpace(liteModel) == "" {
		liteModel = DefaultLiteModel
	}

	// 1. 候補選定 (DSL: LLM)
	selected, err := selectWorlds(ctx, client, model, request)
	if err != nil {
		return nil, fmt.Errorf("world selection failed: %w", err)
	}
	log.Printf("selected %d visited world candidates", len(selected.RecommendedWorlds))

	// 2. 検索 + 抽出 (DSL: イテレーション)
	enriched := enrichWorlds(ctx, client, model, selected.RecommendedWorlds)

	// 3. 推薦モード判定 (DSL: LLM 2)
	recommendationMode, err := judgeRecommendationMode(ctx, client, liteModel, request)
	if err != nil {
		return nil, fmt.Errorf("recommendation mode judgement failed: %w", err)
	}
	wantsUnvisited := recommendationModeWantsUnvisited(recommendationMode)
	log.Printf("recommendation_mode=%s wants_unvisited=%v", recommendationMode, wantsUnvisited)

	// DSL: 検索結果String化 (イテレーション出力の JSON 文字列化)
	enrichedJSON, err := json.MarshalIndent(enriched, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("enriched worlds encode failed: %w", err)
	}

	response := &Response{
		OK:                 true,
		WantsUnvisited:     wantsUnvisited,
		RecommendationMode: recommendationMode,
		VisitedWorlds:      enriched,
		NewWorlds:          []NewWorldCandidate{},
	}

	if recommendationMode == RecommendationModeVisitedOnly {
		// 4a. 訪問済み候補のみで回答 (DSL: LLM 3)
		text, err := generateText(ctx, client, model, visitedOnlyAnswerSystemPrompt,
			visitedOnlyAnswerUserPrompt(request.Query, request.QueryHistory, string(enrichedJSON)), nil)
		if err != nil {
			return nil, fmt.Errorf("visited-only answer failed: %w", err)
		}
		response.Text = text
		return response, nil
	}

	// 4b. 未訪問ワールドのリサーチ (DSL: エージェント; Google Search Grounding で代替)
	researchText, err := generateText(ctx, client, model, researchAgentInstruction,
		researchAgentUserPrompt(request.Query, request.QueryHistory, string(enrichedJSON)),
		[]*genai.Tool{{GoogleSearch: &genai.GoogleSearch{}}})
	if err != nil {
		return nil, fmt.Errorf("new world research failed: %w", err)
	}

	newWorlds := ParseNewWorldCandidates(researchText)
	if len(newWorlds) > maxNewWorldCandidates {
		newWorlds = newWorlds[:maxNewWorldCandidates]
	}
	response.NewWorlds = newWorlds
	if recommendationMode == RecommendationModeUnvisitedOnly {
		response.VisitedWorlds = []EnrichedWorld{}
	}

	// DSL: String化 (選定 structured_output の JSON 文字列化) → LLM 5 入力
	finalAnswerVisitedWorlds := selected
	if recommendationMode == RecommendationModeUnvisitedOnly {
		finalAnswerVisitedWorlds = &SelectedWorlds{RecommendedWorlds: []VisitedWorld{}}
	}
	selectedJSON, err := json.MarshalIndent(finalAnswerVisitedWorlds, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("selected worlds encode failed: %w", err)
	}

	// 4b続き. 最終推薦文 (DSL: LLM 5)
	text, err := generateText(ctx, client, model, finalAnswerSystemPrompt,
		finalAnswerUserPrompt(request.Query, request.QueryHistory, string(selectedJSON), researchText), nil)
	if err != nil {
		return nil, fmt.Errorf("final answer failed: %w", err)
	}
	response.Text = text

	return response, nil
}

// selectWorlds は訪問履歴候補から上位10件を構造化出力で選ぶ (DSL: LLM)。
func selectWorlds(ctx context.Context, client *genai.Client, model string, request *Request) (*SelectedWorlds, error) {
	if len(request.Worlds) == 0 {
		return &SelectedWorlds{RecommendedWorlds: []VisitedWorld{}}, nil
	}

	dbData, err := json.MarshalIndent(map[string]any{"data": request.Worlds}, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("visited worlds encode failed: %w", err)
	}

	schema := &genai.Schema{
		Type:     genai.TypeObject,
		Required: []string{"recommended_worlds"},
		Properties: map[string]*genai.Schema{
			"recommended_worlds": {
				Type:        genai.TypeArray,
				Description: "ユーザーの希望に合うVRChatワールド上位10件。おすすめ度が高い順に並べる。",
				Items: &genai.Schema{
					Type:     genai.TypeObject,
					Required: []string{"world_key", "world_name", "world_id", "visit_count"},
					Properties: map[string]*genai.Schema{
						"world_key":   {Type: genai.TypeString, Description: "入力JSONに含まれていた world_key をそのまま返す。"},
						"world_name":  {Type: genai.TypeString, Description: "入力JSONに含まれていた world_name をそのまま返す。"},
						"world_id":    {Type: genai.TypeString, Nullable: genai.Ptr(true), Description: "入力JSONに含まれていた world_id をそのまま返す。null の場合は null のまま返す。"},
						"visit_count": {Type: genai.TypeInteger, Description: "入力JSONに含まれていた visit_count をそのまま返す。"},
					},
				},
			},
		},
	}

	selected := &SelectedWorlds{}
	err = generateStructured(ctx, client, model, selectWorldsSystemPrompt,
		selectWorldsUserPrompt(request.Query, request.QueryHistory, string(dbData)), schema, selected)
	if err != nil {
		return nil, err
	}

	if len(selected.RecommendedWorlds) > maxSelectedWorlds {
		selected.RecommendedWorlds = selected.RecommendedWorlds[:maxSelectedWorlds]
	}

	return selected, nil
}

// enrichWorlds は選定済み候補ごとに Google Search Grounding で検索し、
// LLM 4 相当の抽出を行う。1件の失敗で全体を止めず matched=false として続行する。
func enrichWorlds(ctx context.Context, client *genai.Client, model string, worlds []VisitedWorld) []EnrichedWorld {
	enriched := make([]EnrichedWorld, len(worlds))
	semaphore := make(chan struct{}, enrichConcurrency)
	var waitGroup sync.WaitGroup

	for index, world := range worlds {
		waitGroup.Add(1)
		go func(index int, world VisitedWorld) {
			defer waitGroup.Done()
			semaphore <- struct{}{}
			defer func() { <-semaphore }()

			enriched[index] = enrichWorld(ctx, client, model, world)
		}(index, world)
	}

	waitGroup.Wait()
	return enriched
}

func enrichWorld(ctx context.Context, client *genai.Client, model string, world VisitedWorld) EnrichedWorld {
	result := EnrichedWorld{VisitedWorld: world}

	// DSL: 検索文作成 + Tavily Search → Google Search Grounding
	searchText, err := generateText(ctx, client, model, "",
		groundedSearchPrompt(world.WorldName),
		[]*genai.Tool{{GoogleSearch: &genai.GoogleSearch{}}})
	if err != nil {
		log.Printf("grounded search failed for %q: %v", world.WorldName, err)
		return result
	}

	// DSL: LLM 4 (構造化抽出)
	schema := &genai.Schema{
		Type:     genai.TypeObject,
		Required: []string{"matched", "world_name", "world_overview", "recommendedNumberOfPeople"},
		Properties: map[string]*genai.Schema{
			"matched":                   {Type: genai.TypeBoolean, Description: "検索結果から対象のVRChatワールドを十分に特定できた場合は true。特定できない、または曖昧な場合は false。"},
			"world_name":                {Type: genai.TypeString, Nullable: genai.Ptr(true), Description: "検索結果から特定したVRChatワールド名。特定できない場合は null。"},
			"world_overview":            {Type: genai.TypeString, Nullable: genai.Ptr(true), Description: "検索結果から確認できるワールド概要。1〜3文で簡潔にまとめる。十分な情報がない場合は null。"},
			"recommendedNumberOfPeople": {Type: genai.TypeInteger, Description: "検索結果から確認できるワールドの推奨人数。十分な情報がない場合はワールドの最大人数。"},
		},
	}

	extraction := &worldExtraction{}
	err = generateStructured(ctx, client, model, extractWorldSystemPrompt,
		extractWorldUserPrompt(world.WorldName, searchText), schema, extraction)
	if err != nil {
		log.Printf("world extraction failed for %q: %v", world.WorldName, err)
		return result
	}

	result.Matched = extraction.Matched
	result.SearchedWorldName = extraction.WorldName
	result.WorldOverview = extraction.WorldOverview
	result.RecommendedNumberOfPeople = extraction.RecommendedNumberOfPeople
	return result
}

func normalizeRecommendationMode(value string) RecommendationMode {
	switch RecommendationMode(strings.TrimSpace(strings.ToLower(value))) {
	case RecommendationModeVisitedOnly:
		return RecommendationModeVisitedOnly
	case RecommendationModeUnvisitedOnly:
		return RecommendationModeUnvisitedOnly
	case RecommendationModeMixed:
		return RecommendationModeMixed
	default:
		return RecommendationModeMixed
	}
}

func recommendationModeWantsUnvisited(mode RecommendationMode) bool {
	switch mode {
	case RecommendationModeVisitedOnly:
		return false
	case RecommendationModeUnvisitedOnly, RecommendationModeMixed:
		return true
	default:
		return true
	}
}

// judgeRecommendationMode は推薦対象を3択で判定する (DSL: LLM 2)。
func judgeRecommendationMode(ctx context.Context, client *genai.Client, model string, request *Request) (RecommendationMode, error) {
	schema := &genai.Schema{
		Type:     genai.TypeObject,
		Required: []string{"recommendation_mode"},
		Properties: map[string]*genai.Schema{
			"recommendation_mode": {
				Type:        genai.TypeString,
				Enum:        []string{string(RecommendationModeVisitedOnly), string(RecommendationModeUnvisitedOnly), string(RecommendationModeMixed)},
				Description: "推薦モード。visited_only / unvisited_only / mixed のいずれか。",
			},
		},
	}

	output := &recommendationModeOutput{}
	err := generateStructured(ctx, client, model, recommendationModeSystemPrompt,
		recommendationModeUserPrompt(request.Query, request.QueryHistory), schema, output)
	if err != nil {
		return RecommendationModeMixed, err
	}

	return normalizeRecommendationMode(output.RecommendationMode), nil
}

// generateText は system prompt / tools 付きでテキスト生成を行う。
func generateText(ctx context.Context, client *genai.Client, model, systemPrompt, userPrompt string, tools []*genai.Tool) (string, error) {
	config := &genai.GenerateContentConfig{Tools: tools}
	if systemPrompt != "" {
		config.SystemInstruction = &genai.Content{Parts: []*genai.Part{{Text: systemPrompt}}}
	}

	result, err := client.Models.GenerateContent(ctx, model, genai.Text(userPrompt), config)
	if err != nil {
		return "", err
	}

	text := result.Text()
	if strings.TrimSpace(text) == "" {
		return "", fmt.Errorf("model %s returned empty text", model)
	}
	return text, nil
}

// generateStructured は response schema 付きで JSON 構造化出力を生成し out に decode する。
// Gemini API は Google Search Grounding と構造化出力を同時指定できないため tools は受け取らない。
func generateStructured(ctx context.Context, client *genai.Client, model, systemPrompt, userPrompt string, schema *genai.Schema, out any) error {
	config := &genai.GenerateContentConfig{
		ResponseMIMEType: "application/json",
		ResponseSchema:   schema,
	}
	if systemPrompt != "" {
		config.SystemInstruction = &genai.Content{Parts: []*genai.Part{{Text: systemPrompt}}}
	}

	result, err := client.Models.GenerateContent(ctx, model, genai.Text(userPrompt), config)
	if err != nil {
		return err
	}

	text := result.Text()
	if strings.TrimSpace(text) == "" {
		return fmt.Errorf("model %s returned empty structured output", model)
	}

	if err := json.Unmarshal([]byte(text), out); err != nil {
		return fmt.Errorf("structured output decode failed: %w", err)
	}
	return nil
}
