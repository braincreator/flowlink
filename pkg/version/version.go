package version

// Версия заполняется при сборке через -ldflags
var (
	Version   = "0.3.1"
	GitCommit = "dev"
	BuildDate = "unknown"
)
