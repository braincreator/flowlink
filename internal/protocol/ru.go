package protocol

// ruMessages — Russian translations for all protocol error codes.
var ruMessages = map[string]string{
	// --- General ---
	CodeOK:                "OK",
	CodeUnknownError:      "Неизвестная ошибка",
	CodeInvalidJSON:       "Неверный JSON",
	CodeInvalidPayload:    "Неверный payload: %v",
	CodeAgentNotFound:     "Агент не найден: %s",
	CodeAgentNotConnected: "Агент не подключён",
	CodeUnauthorized:      "Не авторизован",
	CodeForbidden:         "Доступ запрещён",
	CodeInternalError:     "Внутренняя ошибка",
	CodeUnknownMessage:    "Неизвестный тип сообщения: %s",

	// --- Authentication ---
	CodeTokenMissing:       "Токен не указан",
	CodeTokenInvalid:       "Неверный токен",
	CodeTokenExpired:       "Токен истёк",
	CodeTokenRevoked:       "Токен отозван",
	CodeTokenBlacklisted:   "Токен в чёрном списке",
	CodeSignatureInvalid:   "Неверная подпись",
	CodeSecretNotFound:     "Секрет не найден",
	CodeTokenTypeInvalid:   "Неверный тип токена",
	CodeTokenDecodeFailed:  "Ошибка декодирования токена",
	CodeTokenParseFailed:   "Ошибка парсинга токена",
	CodeTokenSerializeFail: "Ошибка сериализации токена",

	// --- Agent Lifecycle ---
	CodeAgentConnectFailed: "Ошибка подключения агента",
	CodeAgentDisconnected:  "Агент отключён",
	CodeAgentReadFailed:    "Ошибка чтения от агента",
	CodeAgentWriteFailed:   "Ошибка записи агенту",
	CodeAgentNotAuthorized: "Агент не авторизован",
	CodeAgentLimitExceeded: "Лимит агентов превышен (%d/%d)",
	CodeAgentPaused:        "Агент на паузе: %s",
	CodeAgentEmergencyStop: "Экстренная остановка: команды не выполняются",
	CodeProtocolVersionMismatch: "Несовместимая версия протокола: клиент %d, сервер %d",

	// --- Execution ---
	CodeExecSuccess:          "Команда выполнена",
	CodeExecTimeout:          "Таймаут: команда не завершилась за %d сек",
	CodeExecBlocked:          "Команда заблокирована",
	CodeExecBlockedReadOnly:  "Команда заблокирована: агент в режиме только для чтения",
	CodeExecBlockedSandbox:   "Команда заблокирована: совпадение с паттерном",
	CodeExecBlockedSudo:     "Команда заблокирована: sudo не разрешён",
	CodeExecFailed:           "Ошибка выполнения команды",
	CodeExecNeedsApproval:    "Команда требует подтверждения",
	CodeExecAwaitingApproval: "Команда ожидает подтверждения",
	CodeExecRejected:         "Команда отклонена пользователем",
	CodeExecApproved:         "Команда одобрена",

	// --- File Operations ---
	CodeFileEmptyPath:     "Пустой путь",
	CodeFileInvalidPath:   "Неверный путь: %v",
	CodeFileNotFound:      "Файл не найден: %s",
	CodeFileTooLarge:      "Файл слишком большой: %d байт (макс %d байт)",
	CodeFileReadError:     "Ошибка чтения файла: %v",
	CodeFileWriteError:    "Ошибка записи файла: %v",
	CodeFileDecodeError:   "Ошибка декодирования: %v",
	CodeFileDirCreateError: "Ошибка создания директории: %v",
	CodeFileParentDirError: "Ошибка создания родительской директории: %v",
	CodeFileDirReadError:  "Ошибка чтения директории: %v",

	// --- Config ---
	CodeConfigApplied:    "Конфигурация обновлена",
	CodeConfigFailed:     "Ошибка обновления конфигурации",
	CodeConfigLoadError:  "Ошибка загрузки конфига: %v",
	CodeConfigSaveError:  "Ошибка сохранения конфига: %v",
	CodeConfigParseError: "Ошибка парсинга конфига: %v",

	// --- Skills ---
	CodeSkillAlreadyExists: "Скилл %s уже существует (используйте force_update)",
	CodeSkillNotFound:      "Скилл %s не найден",
	CodeSkillSaveError:     "Ошибка сохранения скилла: %v",
	CodeSkillDeleteError:   "Ошибка удаления скилла: %v",
	CodeSkillDirError:      "Ошибка создания директории скиллов: %v",
	CodeSkillSerializeError: "Ошибка сериализации скилла: %v",

	// --- Tasks ---
	CodeTaskAccepted:    "Задача принята",
	CodeTaskError:       "Ошибка задачи: %v",
	CodeTaskCancelError: "Ошибка отмены задачи: %v",
	CodeTaskStepStart:   "Шаг задачи начат",
	CodeTaskStepDone:    "Шаг задачи завершён",
	CodeTaskDone:        "Задача завершена",
	CodeTaskErrorDone:   "Задача завершена с ошибкой",

	// --- Kill Switch ---
	CodeKillSwitchDiskFull:  "Диск почти полон: %.1f%%",
	CodeKillSwitchCPUHigh:   "Высокая загрузка CPU",
	CodeKillSwitchPause:     "Агент на паузе: %s",
	CodeKillSwitchResume:    "Агент возобновил работу",
	CodeKillSwitchEmergency: "Экстренная остановка",

	// --- Backup ---
	CodeBackupEmptyPaths:       "Пустой список путей для бэкапа",
	CodeBackupCreateError:      "Ошибка создания бэкапа: %v",
	CodeBackupArchiveError:     "Ошибка создания архива: %v",
	CodeBackupSnapshotNotFound: "Снапшот не найден: %s",
	CodeBackupRestoreError:     "Ошибка восстановления: %v",
	CodeBackupRestoreOpenError: "Ошибка открытия архива: %v",
	CodeBackupRestoreGzipError: "Ошибка распаковки gzip: %v",
	CodeBackupRestoreReadError: "Ошибка чтения архива: %v",
	CodeBackupRestoreDirError:  "Ошибка создания директории: %v",
	CodeBackupRestoreFileError: "Ошибка записи файла: %v",
	CodeBackupDeleteError:      "Ошибка удаления бэкапа: %v",
	CodeBackupMetadataError:    "Ошибка сохранения метаданных: %v",
	CodeBackupDirCreateError:   "Ошибка создания директории бэкапов: %v",
	CodeBackupSerializeError:   "Ошибка сериализации метаданных: %v",
	CodeBackupGlobError:        "Ошибка glob: %v",
	CodeBackupFileAddError:     "Ошибка добавления в архив: %v",
	CodeBackupCleanup:          "Очистка бэкапов завершена",
	CodeBackupChecksumCompute:  "Ошибка вычисления контрольной суммы: %v",
	CodeBackupChecksumMismatch: "Несовпадение контрольной суммы: ожидалось %s, получено %s",

// --- Audit ---
	CodeAuditDirCreateError:    "Ошибка создания директории audit: %v",
	CodeAuditFileOpenError:     "Ошибка открытия audit файла: %v",
	CodeAuditSerializeError:    "Ошибка сериализации entry: %v",
	CodeAuditWriteError:        "Ошибка записи entry: %v",
	CodeAuditFormatUnsupported: "Неподдерживаемый формат: %s",

	// --- Registry ---
	CodeClientNotFound:    "Клиент не найден: %s",
	CodeClientDeactivated: "Клиент не найден или деактивирован: %s",
	CodeClientCreateError: "Ошибка сохранения клиента: %v",
	CodeClientSaveError:   "Ошибка сохранения: %v",
	CodeClientLoadError:   "Ошибка загрузки: %v",
	CodeRegistryLoadError: "Ошибка загрузки реестра: %v",
	CodeRegistrySaveError: "Ошибка сохранения реестра: %v",
	CodeTokenGenerateError: "Ошибка генерации токена: %v",

	// --- TLS ---
	CodeTLSKeyGenerateError:    "Ошибка генерации ключа: %v",
	CodeTLSSerialGenerateError: "Ошибка генерации серийного номера: %v",
	CodeTLSCertCreateError:     "Ошибка создания сертификата: %v",
	CodeTLSCertDirError:        "Ошибка создания директории для сертификата: %v",
	CodeTLSCertWriteError:      "Ошибка записи сертификата: %v",
	CodeTLSCertLoadError:       "Ошибка загрузки сертификата: %v",
	CodeTLSCertMissing:         "Сертификат отсутствует",
	CodeTLSModeUnknown:         "Неизвестный режим TLS: %s",

	// --- MCP ---
	CodeMCPAgentNotFound: "Агент '%s' не найден (подключено: %d)",
	CodeMCPTimeout:       "Таймаут ожидания ответа от агента (%v)",
	CodeMCPAgentError:    "Ошибка агента: %s",

	// --- LLM ---
	CodeLLMAllBackendsDown: "Все LLM backends недоступны",
	CodeLLMRequestError:    "Ошибка создания запроса: %v",
	CodeLLMResponseError:   "Ошибка чтения ответа: %v",
	CodeLLMEmptyResponse:   "Пустой ответ от LLM",
	CodeLLMParseError:      "Ошибка парсинга ответа: %v",
}
