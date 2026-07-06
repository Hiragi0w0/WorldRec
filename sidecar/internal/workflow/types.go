package workflow

// Request は Tauri (Rust) 側から stdin 経由で受け取る実行リクエスト。
// APIキーは引数・環境変数に漏れないよう stdin JSON でのみ受け渡す。
type Request struct {
	APIKey       string         `json:"api_key"`
	Query        string         `json:"query"`
	QueryHistory string         `json:"query_history"`
	Worlds       []VisitedWorld `json:"visited_worlds"`
	Model        string         `json:"model,omitempty"`
	LiteModel    string         `json:"lite_model,omitempty"`
}

// VisitedWorld は訪問履歴DBから集計したワールド候補1件。
// AI推薦の外部送信用に必要な最小項目だけを含む。
type VisitedWorld struct {
	WorldKey   string  `json:"world_key"`
	WorldName  string  `json:"world_name"`
	WorldID    *string `json:"world_id"`
	VisitCount int64   `json:"visit_count"`
}

// SelectedWorlds は Dify DSL の LLM(候補選定) structured_output と同じ形。
type SelectedWorlds struct {
	RecommendedWorlds []VisitedWorld `json:"recommended_worlds"`
}

// EnrichedWorld は選定済みワールドに検索結果の抽出情報を付与したもの。
// Dify DSL のイテレーション出力 (LLM 4 structured_output) に対応する。
type EnrichedWorld struct {
	VisitedWorld
	Matched                   bool    `json:"matched"`
	SearchedWorldName         *string `json:"searched_world_name"`
	WorldOverview             *string `json:"world_overview"`
	RecommendedNumberOfPeople int64   `json:"recommendedNumberOfPeople"`
}

// worldExtraction は LLM 4 の structured_output スキーマそのもの。
type worldExtraction struct {
	Matched                   bool    `json:"matched"`
	WorldName                 *string `json:"world_name"`
	WorldOverview             *string `json:"world_overview"`
	RecommendedNumberOfPeople int64   `json:"recommendedNumberOfPeople"`
}

// RecommendationMode は AI探索ガイドの推薦対象を表す内部判定。
type RecommendationMode string

const (
	RecommendationModeVisitedOnly   RecommendationMode = "visited_only"
	RecommendationModeUnvisitedOnly RecommendationMode = "unvisited_only"
	RecommendationModeMixed         RecommendationMode = "mixed"
)

// recommendationModeOutput は LLM 2 の structured_output スキーマそのもの。
type recommendationModeOutput struct {
	RecommendationMode string `json:"recommendation_mode"`
}

// NewWorldCandidate はリサーチエージェント出力から抽出した未訪問ワールド候補。
type NewWorldCandidate struct {
	WorldName                 string `json:"world_name"`
	Overview                  string `json:"overview"`
	RecommendedNumberOfPeople string `json:"recommended_number_of_people"`
}

// Response は stdout 経由で Tauri (Rust) 側へ返す実行結果。
type Response struct {
	OK                 bool                `json:"ok"`
	Error              string              `json:"error,omitempty"`
	ErrorKind          string              `json:"error_kind,omitempty"`
	Text               string              `json:"text"`
	WantsUnvisited     bool                `json:"wants_unvisited"`
	RecommendationMode RecommendationMode  `json:"recommendation_mode"`
	VisitedWorlds      []EnrichedWorld     `json:"visited_worlds"`
	NewWorlds          []NewWorldCandidate `json:"new_worlds"`
}
