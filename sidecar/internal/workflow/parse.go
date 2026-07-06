package workflow

import "strings"

// ParseNewWorldCandidates はリサーチエージェントの出力
// (「ワールド名: ...」「概要: ...」「推奨人数: ...」の空行区切りブロック) を
// UIカード表示用の構造化データに変換する。
// 形式に沿わない部分は無視し、ワールド名を持つブロックだけを返す。
func ParseNewWorldCandidates(text string) []NewWorldCandidate {
	candidates := []NewWorldCandidate{}
	blocks := strings.Split(strings.ReplaceAll(text, "\r\n", "\n"), "\n\n")

	for _, block := range blocks {
		candidate := NewWorldCandidate{}
		for _, line := range strings.Split(block, "\n") {
			line = strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(line), "・"))
			if value, ok := fieldValue(line, "ワールド名"); ok {
				candidate.WorldName = value
			} else if value, ok := fieldValue(line, "概要"); ok {
				candidate.Overview = value
			} else if value, ok := fieldValue(line, "推奨人数"); ok {
				candidate.RecommendedNumberOfPeople = value
			}
		}
		if candidate.WorldName != "" {
			candidates = append(candidates, candidate)
		}
	}

	return candidates
}

// fieldValue は「ラベル: 値」(全角コロンも許容) 形式の行から値を取り出す。
func fieldValue(line, label string) (string, bool) {
	rest, found := strings.CutPrefix(line, label)
	if !found {
		return "", false
	}
	rest = strings.TrimSpace(rest)
	if value, ok := strings.CutPrefix(rest, ":"); ok {
		return strings.TrimSpace(value), true
	}
	if value, ok := strings.CutPrefix(rest, "："); ok {
		return strings.TrimSpace(value), true
	}
	return "", false
}
