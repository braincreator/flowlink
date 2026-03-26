package version

// Версия заполняется при сборке через -ldflags
var (
	Version   = "0.1.0"
	GitCommit = "dev"
	BuildDate = "unknown"
)
