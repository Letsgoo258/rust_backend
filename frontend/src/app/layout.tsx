import type { Metadata } from "next";
import "./globals.css";
import FaroProvider from "@/components/FaroProvider";
import { AnalyticsProvider } from "@/components/providers/AnalyticsProvider";

export const metadata: Metadata = {
  title: "ERAVAYA ERP",
  description: "Minimalist School Management System",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="font-sans antialiased bg-[#F5F5F7] text-[#1D1D1F]">
        <FaroProvider>
          <AnalyticsProvider>
            {children}
          </AnalyticsProvider>
        </FaroProvider>
      </body>
    </html>
  );
}
