// Command fundament-plugin is the Fundament plugin that runs the regelrecht
// RegelHet for one case. It is derived from the Haven+ plugin
// (gitlab.com/commonground/haven/havenplus/plugin-fundament, EUPL-1.2).
package main

import (
	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
)

func main() {
	pluginruntime.Run(NewRegelHetPlugin())
}
