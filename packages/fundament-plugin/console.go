package main

import (
	"embed"
	"net/http"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/console"
)

//go:embed console/*
var consoleFiles embed.FS

// ConsoleAssets makes the plugin a ConsoleProvider, which is what registers
// the /console/ route in pluginruntime.Run; without it the console's iframes
// stay empty. RequireHTML: the pages are hand-written and checked in, so a
// missing file means the embed pattern broke, and the plugin fails at start
// rather than with a blank page.
func (p *RegelHetPlugin) ConsoleAssets() http.FileSystem {
	return console.NewFileSystem(consoleFiles, "console", console.RequireHTML())
}
