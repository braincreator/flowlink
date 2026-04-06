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
  title: "FlowLink — Ctrl+Z для твоего сервера",
  description:
    "AI-native targeted undo для серверных команд. Автоматический бекап перед опасными командами — только затронутые файлы. Восстановление за секунды. E2EE шифрование.",
  openGraph: {
    title: "FlowLink — Ctrl+Z для твоего сервера",
    description:
      "AI автоматически бекапит только то, что может сломаться — и восстанавливает за секунды.",
    type: "website",
    locale: "ru_RU",
    siteName: "FlowLink",
  },
  twitter: {
    card: "summary_large_image",
    title: "FlowLink — Ctrl+Z для твоего сервера",
    description:
      "AI автоматически бекапит только то, что может сломаться — и восстанавливает за секунды.",
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
                    "AI-native targeted undo для серверных команд. Автоматический бекап перед опасными командами.",
                  offers: [
                    {
                      "@type": "Offer",
                      price: "0",
                      priceCurrency: "RUB",
                      name: "Free",
                    },
                    {
                      "@type": "Offer",
                      price: "990",
                      priceCurrency: "RUB",
                      name: "Starter",
                    },
                    {
                      "@type": "Offer",
                      price: "4990",
                      priceCurrency: "RUB",
                      name: "Business",
                    },
                  ],
                },
                {
                  "@type": "FAQPage",
                  mainEntity: [
                    {
                      "@type": "Question",
                      name: "Чем FlowLink отличается от обычного бекапа?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "FlowLink бекапит НЕ весь VPS, а только файлы под угрозой — килобайты вместо гигабайт. Бекап создаётся автоматически перед опасной командой, а восстановление занимает секунды.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Насколько безопасно?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "E2EE шифрование X25519 + AES-256-GCM. Даже relay-сервер не имеет доступа к вашим данным. Все команды проходят через Policy Layer с blacklist, sandbox и approval.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Сколько места занимает?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Typical snapshot — 10KB-50MB (только затронутые файлы, tar.gz). Автоматическая очистка старых снапшотов. Максимальный лимит настраивается.",
                      },
                    },
                    {
                      "@type": "Question",
                      name: "Что если агент упадёт?",
                      acceptedAnswer: {
                        "@type": "Answer",
                        text: "Снапшоты хранятся локально на сервере. Даже если FlowLink агент недоступен, все бекапы на месте и могут быть восстановлены вручную.",
                      },
                    },
                  ],
                },
              ],
            }),
          }}
        />
      </head>
      <body className="min-h-screen bg-[#0a0a0a] text-[#e0e0e0] antialiased">
        {children}
      </body>
    </html>
  );
}
