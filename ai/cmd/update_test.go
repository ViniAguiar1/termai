package cmd

import "testing"

func TestCompareVersions(t *testing.T) {
	cases := []struct {
		a, b string
		want int
	}{
		{"0.2.0", "0.1.0", 1},
		{"0.1.0", "0.2.0", -1},
		{"1.0.0", "1.0.0", 0},
		{"v0.2.0", "0.1.9", 1},   // 'v' prefix ignored
		{"0.1.0", "0.1.0-beta", 0}, // build/pre-release suffix ignored
		{"0.10.0", "0.9.0", 1},   // numeric, not lexical
		{"1.2", "1.2.0", 0},      // missing parts treated as 0
		{"0.1.0", "dev", 1},      // unparseable current => treated as 0.0.0
	}
	for _, c := range cases {
		if got := compareVersions(c.a, c.b); got != c.want {
			t.Errorf("compareVersions(%q, %q) = %d, want %d", c.a, c.b, got, c.want)
		}
	}
}
