// worldrec-ai は WorldRec の AI ワールド推薦ワークフローを実行する sidecar。
//
// stdin から JSON リクエスト (api_key, query, query_history, visited_worlds) を受け取り、
// stdout に JSON レスポンスを1行で書き出す。ログは stderr にのみ出力し、
// APIキーなどの秘密情報はログに書かない。
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"time"

	"github.com/Hiragi0w0/WorldRec-dev/sidecar/internal/workflow"
)

const (
	maxRequestBytes = 10 << 20 // 10 MiB
	runTimeout      = 4 * time.Minute
)

func main() {
	log.SetOutput(os.Stderr)
	log.SetPrefix("worldrec-ai: ")

	response := run()

	encoder := json.NewEncoder(os.Stdout)
	if err := encoder.Encode(response); err != nil {
		log.Printf("response encode failed: %v", err)
		os.Exit(1)
	}

	if !response.OK {
		os.Exit(2)
	}
}

func run() *workflow.Response {
	request, err := readRequest(os.Stdin)
	if err != nil {
		return errorResponse(
			fmt.Sprintf("リクエストの読み取りに失敗しました: %v", err),
			workflow.ErrorKindInvalidResponse,
		)
	}

	ctx, cancel := context.WithTimeout(context.Background(), runTimeout)
	defer cancel()

	response, err := workflow.Run(ctx, request)
	if err != nil {
		log.Printf("workflow failed: %v", err)
		return errorResponse(
			fmt.Sprintf("AIワークフローの実行に失敗しました: %v", err),
			workflow.ClassifyError(err),
		)
	}

	return response
}

func readRequest(reader io.Reader) (*workflow.Request, error) {
	raw, err := io.ReadAll(io.LimitReader(reader, maxRequestBytes))
	if err != nil {
		return nil, err
	}

	request := &workflow.Request{}
	if err := json.Unmarshal(raw, request); err != nil {
		return nil, err
	}
	return request, nil
}

func errorResponse(message string, errorKind string) *workflow.Response {
	return &workflow.Response{
		OK:            false,
		Error:         message,
		ErrorKind:     errorKind,
		VisitedWorlds: []workflow.EnrichedWorld{},
		NewWorlds:     []workflow.NewWorldCandidate{},
	}
}
