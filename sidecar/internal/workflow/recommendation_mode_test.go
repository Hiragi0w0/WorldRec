package workflow

import "testing"

func TestNormalizeRecommendationMode(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  RecommendationMode
	}{
		{name: "visited only", input: "visited_only", want: RecommendationModeVisitedOnly},
		{name: "unvisited only", input: "unvisited_only", want: RecommendationModeUnvisitedOnly},
		{name: "mixed", input: "mixed", want: RecommendationModeMixed},
		{name: "trims and lowercases", input: " MIXED ", want: RecommendationModeMixed},
		{name: "unknown falls back to mixed", input: "unknown", want: RecommendationModeMixed},
		{name: "empty falls back to mixed", input: "", want: RecommendationModeMixed},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if got := normalizeRecommendationMode(test.input); got != test.want {
				t.Fatalf("normalizeRecommendationMode(%q) = %q, want %q", test.input, got, test.want)
			}
		})
	}
}

func TestRecommendationModeWantsUnvisited(t *testing.T) {
	tests := []struct {
		mode RecommendationMode
		want bool
	}{
		{mode: RecommendationModeVisitedOnly, want: false},
		{mode: RecommendationModeUnvisitedOnly, want: true},
		{mode: RecommendationModeMixed, want: true},
		{mode: RecommendationMode("unexpected"), want: true},
	}

	for _, test := range tests {
		t.Run(string(test.mode), func(t *testing.T) {
			if got := recommendationModeWantsUnvisited(test.mode); got != test.want {
				t.Fatalf("recommendationModeWantsUnvisited(%q) = %v, want %v", test.mode, got, test.want)
			}
		})
	}
}
