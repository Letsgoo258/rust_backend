import type { Metadata } from "next";
import { Inter } from "next/font/google";
import "./globals.css";
import FaroProvider from "@/components/FaroProvider";

// Apple-style minimal font choice
const inter = Inter({
  subsets: ["latin"],
  variable: "--font-inter",
  display: "swap",
});

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
      <body className={`${inter.variable} font-sans antialiased bg-[#F5F5F7] text-[#1D1D1F]`}>
        <FaroProvider>
          {children}
        </FaroProvider>
      </body>
    </html>
  );
}
