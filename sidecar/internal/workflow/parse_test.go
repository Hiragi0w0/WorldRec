package workflow

import "testing"

func TestParseNewWorldCandidates(t *testing.T) {
	text := `ワールド名: Cozy Cafe World
概要: 落ち着いた雰囲気のカフェワールド。
推奨人数: 4

ワールド名: Neon City
概要: サイバーパンクな夜景の街。
推奨人数: 不明`

	candidates := ParseNewWorldCandidates(text)

	if len(candidates) != 2 {
		t.Fatalf("expected 2 candidates, got %d", len(candidates))
	}
	if candidates[0].WorldName != "Cozy Cafe World" {
		t.Errorf("unexpected world name: %q", candidates[0].WorldName)
	}
	if candidates[0].Overview != "落ち着いた雰囲気のカフェワールド。" {
		t.Errorf("unexpected overview: %q", candidates[0].Overview)
	}
	if candidates[0].RecommendedNumberOfPeople != "4" {
		t.Errorf("unexpected recommended people: %q", candidates[0].RecommendedNumberOfPeople)
	}
	if candidates[1].RecommendedNumberOfPeople != "不明" {
		t.Errorf("unexpected recommended people: %q", candidates[1].RecommendedNumberOfPeople)
	}
}

func TestParseNewWorldCandidatesFullWidthColonAndBullets(t *testing.T) {
	text := `・ワールド名： 森の隠れ家
・概要： 森の中の静かな家。
・推奨人数： 2`

	candidates := ParseNewWorldCandidates(text)

	if len(candidates) != 1 {
		t.Fatalf("expected 1 candidate, got %d", len(candidates))
	}
	if candidates[0].WorldName != "森の隠れ家" {
		t.Errorf("unexpected world name: %q", candidates[0].WorldName)
	}
}

func TestParseNewWorldCandidatesIgnoresNoise(t *testing.T) {
	text := `以下が候補です。

概要: ワールド名がないブロックは無視される。

ワールド名: Valid World
概要: 有効な候補。
推奨人数: 8`

	candidates := ParseNewWorldCandidates(text)

	if len(candidates) != 1 {
		t.Fatalf("expected 1 candidate, got %d", len(candidates))
	}
	if candidates[0].WorldName != "Valid World" {
		t.Errorf("unexpected world name: %q", candidates[0].WorldName)
	}
}

func TestParseNewWorldCandidatesEmpty(t *testing.T) {
	if got := ParseNewWorldCandidates(""); len(got) != 0 {
		t.Fatalf("expected no candidates, got %d", len(got))
	}
}
