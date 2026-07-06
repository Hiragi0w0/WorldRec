package workflow

import (
	"encoding/json"
	"testing"
)

func TestVisitedWorldJSONExcludesMemoAndTags(t *testing.T) {
	worldID := "wrld_a"
	world := VisitedWorld{
		WorldKey:   "wrld_a",
		WorldName:  "World A",
		WorldID:    &worldID,
		VisitCount: 3,
	}

	payload, err := json.Marshal(world)
	if err != nil {
		t.Fatalf("VisitedWorld marshal failed: %v", err)
	}

	var fields map[string]any
	if err := json.Unmarshal(payload, &fields); err != nil {
		t.Fatalf("VisitedWorld JSON decode failed: %v", err)
	}

	if _, ok := fields["tags"]; ok {
		t.Fatalf("VisitedWorld JSON contains tags: %s", payload)
	}
	if _, ok := fields["memo"]; ok {
		t.Fatalf("VisitedWorld JSON contains memo: %s", payload)
	}
}
