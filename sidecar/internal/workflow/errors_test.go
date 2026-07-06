package workflow

import (
	"context"
	"fmt"
	"testing"

	"google.golang.org/genai"
)

func TestClassifyError(t *testing.T) {
	tests := []struct {
		name string
		err  error
		want string
	}{
		{
			name: "nil",
			err:  nil,
			want: "",
		},
		{
			name: "deadline exceeded",
			err:  context.DeadlineExceeded,
			want: ErrorKindTimeout,
		},
		{
			name: "api error 401",
			err:  fmt.Errorf("wrapped: %w", genai.APIError{Code: 401, Message: "bad key"}),
			want: ErrorKindInvalidAPIKey,
		},
		{
			name: "api error pointer 403",
			err:  fmt.Errorf("wrapped: %w", &genai.APIError{Code: 403, Message: "permission denied"}),
			want: ErrorKindInvalidAPIKey,
		},
		{
			name: "api error 429",
			err:  fmt.Errorf("wrapped: %w", genai.APIError{Code: 429, Message: "quota"}),
			want: ErrorKindRateLimited,
		},
		{
			name: "api error 503",
			err:  fmt.Errorf("wrapped: %w", genai.APIError{Code: 503, Message: "unavailable"}),
			want: ErrorKindServiceUnavailable,
		},
		{
			name: "decode failed",
			err:  fmt.Errorf("structured output decode failed: invalid character"),
			want: ErrorKindInvalidResponse,
		},
		{
			name: "connection refused",
			err:  fmt.Errorf("dial tcp 127.0.0.1:443: connection refused"),
			want: ErrorKindNetwork,
		},
		{
			name: "authentication message",
			err:  fmt.Errorf("authentication failed: invalid API key"),
			want: ErrorKindInvalidAPIKey,
		},
		{
			name: "unknown",
			err:  fmt.Errorf("unexpected workflow failure"),
			want: ErrorKindUnknown,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ClassifyError(tt.err); got != tt.want {
				t.Fatalf("ClassifyError() = %q, want %q", got, tt.want)
			}
		})
	}
}
