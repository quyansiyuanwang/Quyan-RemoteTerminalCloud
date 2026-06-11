//go:build !windows

package main

import "fmt"

func main() {
	fmt.Println("rtc-agent-manager is only available on Windows.")
}
