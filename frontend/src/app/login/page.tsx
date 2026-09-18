"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

export default function LoginPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");

  const handleLogin = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading(true);
    setErrorMsg("");

    const formData = new FormData(e.currentTarget);
    const data = Object.fromEntries(formData.entries());

    try {
      const res = await fetch("/api/v1/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
      });

      if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || "Login failed");
      }
      
      const userRes = await res.json();
      // If SUPER_ADMIN, redirect to super-admin dashboard
      if (userRes.user?.user_type === "SUPER_ADMIN") {
        router.push("/dashboard/super-admin");
      } else {
        router.push("/dashboard");
      }
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-4 bg-[#FAFAFA]">
      <div className="w-full max-w-[400px] bg-white border border-gray-100 shadow-[0_4px_24px_rgba(0,0,0,0.02)] rounded-2xl p-8 z-10 relative">
        <div className="mb-8">
          <div className="w-10 h-10 bg-[#0C152E] rounded-lg mb-6 flex items-center justify-center">
            <span className="text-white font-bold text-lg">E</span>
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-gray-900 mb-1">Welcome back</h1>
          <p className="text-gray-500 text-sm">Please enter your details to sign in.</p>
        </div>

        {errorMsg && (
          <div className="mb-6 p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg">
            {errorMsg}
          </div>
        )}

        <form onSubmit={handleLogin} className="space-y-5">
          <div className="space-y-1.5">
            <label className="block text-xs font-medium text-gray-700 uppercase tracking-wide">Username / Email</label>
            <input required name="username" type="text" placeholder="admin@school.com" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2.5 text-sm transition-all" />
          </div>

          <div className="space-y-1.5 pb-2">
            <label className="block text-xs font-medium text-gray-700 uppercase tracking-wide">Password</label>
            <input required name="password" type="password" placeholder="••••••••" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2.5 text-sm transition-all" />
          </div>

          <button disabled={loading} type="submit" className="w-full bg-[#3366FF] hover:bg-[#2B57D9] disabled:bg-[#3366FF]/70 text-white font-medium py-2.5 rounded-lg transition-all text-sm shadow-sm">
            {loading ? "Signing in..." : "Sign in"}
          </button>
        </form>
      </div>
    </div>
  );
}
