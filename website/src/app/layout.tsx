import type { Metadata } from "next";
import { Inter, JetBrains_Mono } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-sans",
  subsets: ["latin", "cyrillic"],
  display: "swap",
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-mono",
  subsets: ["latin"],
  display: "swap",
});

export const metadata: Metadata = {
  title: "FlowLink — AI Security Shield для серверов",
  description:
    "Защита серверов от AI-агентов. Kernel-level перехват опасных команд, E2EE шифрование, auto-backup, GitOps rollback. Rust agent, K8s operator.",
  openGraph: {
    title: "FlowLink — AI Security Shield для серверов",
    description:
      "Перехватывает, анализирует и блокирует опасные команды AI-агентов на kernel-level. E2EE, auto-backup, GitOps.",
    type: "website",
    locale: "ru_RU",
    siteName: "FlowLink",
  },
  twitter: {
    card: "summary_large_image",
    title: "FlowLink — AI Security Shield для серверов",
    description:
      "Перехватывает, анализирует и блокирует опасные команды AI-агентов на kernel-level.",
  },
  robots: {
    index: true,
    follow: true,
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ru" className={`${inter.variable} ${jetbrainsMono.variable}`}>
      <head>
        <link rel="icon" type="image/svg+xml" href="/logo.svg" />
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{
            __html: JSON.stringify({
              "@context": "https://schema.org",
              "@graph": [
                {
                  "@type": "SoftwareApplication",
                  name: "FlowLink",
                  applicationCategory: "DeveloperApplication",
                  operatingSystem: "Linux",
                  description:
                    "AI Security Shield для серверов. Kernel-level перехват опасных команд, E2EE шифрование, auto-backup, GitOps rollback.",
                  offers: [
                    {
                      "@type": "Offer",
                      price: "0",
                      priceCurrency: "RUB",
                      name: "Trial",
                      description: "7 дней бесплатно, 1 хост, pattern blocking, E2EE",
                    },
                    {
                      "@type": "Offer",
                      price: "990",
                      priceCurrency: "RUB",
                      name: "Starter",
                      description: "3 хоста, AST-анализ, Telegram бот, dashboard",
                    },
                    {
                      "@type": "Offer",
                      price: "4990",
                      priceCurrency: "RUB",
                      name: "Pro",
                      description: "25 хостов, eBPF kernel-level, K8s operator, GitOps, SIEM",
                    },
                  ],
                },
                {
                  "@type": "FAQPage",
                  mainEntity: [
                    {
                      "@type": "Question",
                      name: "Зачем FlowLink, если есть Falco / OPA / sudo?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Falco = runtime alerting (не блокирует). OPA = policy как код (сложно). sudo = insufficient для AI. FlowLink = AI-native: понимает контекст, блокирует на kernel-level, auto-бэкапит.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Насколько это безопасно?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "E2EE шифрование X25519 + AES-256-GCM. Приватные ключи хранятся только на агенте — relay-сервер не может расшифровать данные.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Сколько ресурсов жрёт агент?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Rust binary: ~15MB RAM idle, ~50MB при пике. CPU: <1% при мониторинге.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Что если агент упадёт?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Локальные бэкапы хранятся на сервере. Даже если relay недоступен, все бэкапы на месте.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Какие команды блокируются?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "50+ паттернов в 4 категориях + AST-анализ обфускации + eBPF syscall interception.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Сколько агентов можно подключить?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Без ограничений. FlowLink защищает хосты, а не агенты.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Как trial работает?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "7 дней бесплатно: 1 хост, pattern blocking, E2EE. Без карты.",
                      },
                    },
                  ],
                },
              ],
            }),
          }}
        />
      </head>
      <body className="min-h-screen bg-[#050510] text-[#f0f0ff] antialiased">
        {children}
      </body>
    </html>
  );
}
