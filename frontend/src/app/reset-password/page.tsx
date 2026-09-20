"use client";

import { useState, useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";

export default function ResetPasswordPage() {
  const router = useRouter();
  const searchParams = useSearchParams();

  const [step, setStep] = useState<"REQUEST" | "CONFIRM">("REQUEST");
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const [errorMsg, setErrorMsg] = useState("");

  useEffect(() => {
    const queryEmail = searchParams.get("email");
    const queryToken = searchParams.get("token");
    if (queryEmail) {
      setEmail(queryEmail);
      setStep("CONFIRM");
    }
  }, [searchParams]);

  const handleRequestReset = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email) return;

    setLoading(true);
    setErrorMsg("");
    setMessage("");

    try {
      const res = await fetch("/api/v1/auth/password-reset/request", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Failed to request password reset");

      setMessage(data.message || "Reset link and code sent to your email.");
      setStep("CONFIRM");
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleConfirmReset = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email || !code || !newPassword) {
      setErrorMsg("Please fill in all fields");
      return;
    }

    setLoading(true);
    setErrorMsg("");
    setMessage("");

    try {
      const res = await fetch("/api/v1/auth/password-reset/confirm", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          email,
          code,
          new_password: newPassword,
        }),
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Failed to reset password");

      setMessage("Password successfully reset! Redirecting to login...");
      setTimeout(() => {
        router.push("/login");
      }, 2000);
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex w-full bg-white font-sans overflow-hidden">
      {/* LEFT COLUMN: Graphic / Brand Side */}
      <div className="hidden lg:flex flex-col relative w-1/2 bg-gradient-to-br from-[#E3F2FD] via-[#E1F5FE] to-[#E0F7FA] border-r border-gray-200">
        <div className="absolute inset-0 opacity-40 bg-[url('https://grainy-gradients.vercel.app/noise.svg')] mix-blend-overlay"></div>
        <div className="relative z-10 p-12 h-full flex flex-col justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-[#0C152E] rounded-md flex items-center justify-center">
              <span className="text-white font-bold text-sm">E</span>
            </div>
            <span className="font-semibold tracking-tight text-xl text-[#0C152E]">Eravaya</span>
          </div>

          <div className="max-w-md pb-10">
            <h2 className="text-[32px] font-bold text-[#0C152E] leading-tight mb-8">
              Account Security &amp; Password Recovery
            </h2>
            <p className="text-[#0C152E]/70 text-sm">
              Enter your registered email address to receive a secure signed reset link and 6-digit verification code.
            </p>
          </div>
        </div>
      </div>

      {/* RIGHT COLUMN: Form Side */}
      <div className="flex-1 flex flex-col justify-center px-6 sm:px-12 lg:px-24 xl:px-32 relative">
        <div className="w-full max-w-[420px] mx-auto">
          <div className="mb-6">
            <div className="w-10 h-10 bg-[#3366FF] rounded-lg mb-4 flex items-center justify-center shadow-sm">
              <span className="text-white font-bold text-xl">E</span>
            </div>
            <h1 className="text-3xl font-bold tracking-tight text-gray-900 leading-tight">
              Reset Password
            </h1>
            <p className="text-gray-500 text-sm mt-1">
              {step === "REQUEST"
                ? "Enter your account email to receive a reset code."
                : "Enter the verification code and your new password."}
            </p>
          </div>

          {errorMsg && (
            <div className="mb-6 p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg">
              {errorMsg}
            </div>
          )}

          {message && (
            <div className="mb-6 p-3 bg-green-50 border border-green-100 text-green-700 text-sm rounded-lg font-medium">
              {message}
            </div>
          )}

          {step === "REQUEST" ? (
            <form onSubmit={handleRequestReset} className="space-y-4">
              <Input
                required
                type="email"
                placeholder="Enter your registered email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />

              <Button type="submit" loading={loading} loadingText="Sending Code..." fullWidth className="mt-2">
                Send Reset Code
              </Button>
            </form>
          ) : (
            <form onSubmit={handleConfirmReset} className="space-y-4">
              <Input
                required
                type="email"
                placeholder="Registered email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />

              <Input
                required
                type="text"
                placeholder="6-digit Verification Code"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />

              <Input
                required
                type="password"
                placeholder="New Password (min. 8 characters)"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
              />

              <Button type="submit" loading={loading} loadingText="Updating Password..." fullWidth className="mt-2">
                Set New Password
              </Button>

              <button
                type="button"
                onClick={() => setStep("REQUEST")}
                className="w-full text-center text-xs text-[#3366FF] hover:underline font-medium pt-2"
              >
                Resend code / Change email
              </button>
            </form>
          )}

          <div className="mt-8 text-center">
            <Link href="/login" className="text-sm text-gray-600 hover:text-gray-900 font-medium">
              &larr; Back to Login
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
