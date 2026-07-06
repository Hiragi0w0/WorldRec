package workflow

import (
	"context"
	"errors"
	"strings"

	"google.golang.org/genai"
)

// ErrorKind constants are part of the Rust side contract. Do not rename them.
const (
	ErrorKindInvalidAPIKey      = "invalid_api_key"
	ErrorKindServiceUnavailable = "service_unavailable"
	ErrorKindTimeout            = "timeout"
	ErrorKindRateLimited        = "rate_limited"
	ErrorKindInvalidResponse    = "invalid_response"
	ErrorKindNetwork            = "network"
	ErrorKindUnknown            = "unknown"
)

// ClassifyError classifies workflow execution errors into stable ErrorKind strings.
func ClassifyError(err error) string {
	if err == nil {
		return ""
	}

	message := strings.ToLower(err.Error())
	if errors.Is(err, context.DeadlineExceeded) ||
		containsAny(message, "deadline exceeded", "timeout", "timed out") {
		return ErrorKindTimeout
	}

	var apiError *genai.APIError
	if errors.As(err, &apiError) && apiError != nil {
		if kind := classifyAPIErrorCode(apiError.Code); kind != "" {
			return kind
		}
	}

	var apiErrorValue genai.APIError
	if errors.As(err, &apiErrorValue) {
		if kind := classifyAPIErrorCode(apiErrorValue.Code); kind != "" {
			return kind
		}
	}

	switch {
	case containsAny(message,
		"api key not valid",
		"api_key_invalid",
		"invalid api key",
		"unauthorized",
		"unauthenticated",
		"permission denied",
		"authentication",
	):
		return ErrorKindInvalidAPIKey
	case containsAny(message,
		"resource_exhausted",
		"rate limit",
		"quota",
		"429",
	):
		return ErrorKindRateLimited
	case containsAny(message,
		"unavailable",
		"overloaded",
		"503",
		"502",
		"500",
		"internal server error",
	):
		return ErrorKindServiceUnavailable
	case containsAny(message,
		"structured output decode failed",
		"returned empty",
		"unmarshal",
		"parse",
	):
		return ErrorKindInvalidResponse
	case containsAny(message,
		"connection refused",
		"no such host",
		"network",
		"dial tcp",
		"connection reset",
		"eof",
	):
		return ErrorKindNetwork
	default:
		return ErrorKindUnknown
	}
}

func classifyAPIErrorCode(code int) string {
	switch code {
	case 401, 403:
		return ErrorKindInvalidAPIKey
	case 429:
		return ErrorKindRateLimited
	case 500, 502, 503, 504:
		return ErrorKindServiceUnavailable
	default:
		return ""
	}
}

func containsAny(value string, needles ...string) bool {
	for _, needle := range needles {
		if strings.Contains(value, needle) {
			return true
		}
	}
	return false
}
