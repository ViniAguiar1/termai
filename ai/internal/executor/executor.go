package executor

import (
	"bytes"
	"errors"
	"io"
	"os"
	"os/exec"
	"strings"
	"sync"
	"syscall"

	"github.com/creack/pty"
)

type Result struct {
	Output   string
	Error    string
	ExitCode int
}

var (
	sessionEnvMu sync.RWMutex
	sessionEnv   = map[string]string{}
)

func Run(command string) Result {
	return runCommand(command)
}

func RunWithPTY(command string) Result {
	cmd := exec.Command("sh", "-c", command)
	cmd.Env = interactiveEnv()

	ptmx, err := pty.Start(cmd)
	if err != nil {
		return Result{
			Error:    err.Error(),
			ExitCode: 1,
		}
	}
	defer func() { _ = ptmx.Close() }()

	output, readErr := io.ReadAll(ptmx)
	waitErr := cmd.Wait()

	result := Result{
		Output:   string(output),
		ExitCode: 0,
	}

	if readErr != nil && !errors.Is(readErr, syscall.EIO) {
		result.Error = readErr.Error()
		result.ExitCode = 1
		return result
	}

	if waitErr != nil {
		if exitErr, ok := waitErr.(*exec.ExitError); ok {
			result.ExitCode = exitErr.ExitCode()
		} else {
			result.ExitCode = 1
		}

		result.Error = result.Output
	}

	return result
}

func RunAndUpdateSession(command string) Result {
	const envMarker = "\n__TERMAI_ENV_START__\n"

	result := runCommand(command + `; __termai_exit=$?; printf '` + envMarker + `'; env -0; exit "$__termai_exit"`)
	if result.ExitCode != 0 {
		result.Output = stripEnvPayload(result.Output, envMarker)
		return result
	}

	output, envData, found := strings.Cut(result.Output, envMarker)
	result.Output = output

	if found {
		updateSessionEnv(envData)
	}

	return result
}

func runCommand(command string) Result {
	cmd := exec.Command("sh", "-c", command)
	cmd.Env = mergedEnv()

	var out bytes.Buffer
	var stderr bytes.Buffer

	cmd.Stdout = &out
	cmd.Stderr = &stderr

	err := cmd.Run()

	result := Result{
		Output:   out.String(),
		Error:    stderr.String(),
		ExitCode: 0,
	}

	if err != nil {
		// pega exit code real
		if exitErr, ok := err.(*exec.ExitError); ok {
			result.ExitCode = exitErr.ExitCode()
		} else {
			result.ExitCode = 1
		}
	}

	return result
}

func mergedEnv() []string {
	sessionEnvMu.RLock()
	defer sessionEnvMu.RUnlock()

	envMap := make(map[string]string)
	for _, item := range os.Environ() {
		key, value, ok := strings.Cut(item, "=")
		if ok {
			envMap[key] = value
		}
	}

	for key, value := range sessionEnv {
		envMap[key] = value
	}

	merged := make([]string, 0, len(envMap))
	for key, value := range envMap {
		merged = append(merged, key+"="+value)
	}

	return merged
}

func interactiveEnv() []string {
	env := mergedEnv()

	for _, item := range env {
		if strings.HasPrefix(item, "TERM=") {
			return env
		}
	}

	return append(env, "TERM=xterm-256color")
}

func updateSessionEnv(raw string) {
	items := strings.Split(raw, "\x00")

	sessionEnvMu.Lock()
	defer sessionEnvMu.Unlock()

	for _, item := range items {
		if item == "" {
			continue
		}

		key, value, ok := strings.Cut(item, "=")
		if !ok {
			continue
		}

		sessionEnv[key] = value
	}
}

func stripEnvPayload(output string, marker string) string {
	cleaned, _, found := strings.Cut(output, marker)
	if !found {
		return output
	}

	return cleaned
}
