package cmd

import (
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/chzyer/readline"
)

type lineReader interface {
	ReadLine(prompt string) (string, error)
	Close() error
}

type readlineReader struct {
	rl *readline.Instance
}

func newLineReader() (lineReader, error) {
	config := &readline.Config{
		Prompt:          "",
		InterruptPrompt: "^C",
		EOFPrompt:       "exit",
		HistoryFile:     historyFilePath(),
	}

	rl, err := readline.NewEx(config)
	if err != nil {
		return nil, err
	}

	return &readlineReader{rl: rl}, nil
}

func (r *readlineReader) ReadLine(prompt string) (string, error) {
	r.rl.SetPrompt(prompt)

	line, err := r.rl.Readline()
	if err != nil {
		if errors.Is(err, readline.ErrInterrupt) {
			return "", nil
		}

		if errors.Is(err, io.EOF) {
			return "", err
		}

		return "", err
	}

	return strings.TrimSpace(line), nil
}

func (r *readlineReader) Close() error {
	return r.rl.Close()
}

func historyFilePath() string {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(os.TempDir(), ".termai_history")
	}

	return filepath.Join(homeDir, ".termai_history")
}
