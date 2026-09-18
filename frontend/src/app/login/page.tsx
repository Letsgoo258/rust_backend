"use client";

import { useState } from "react";

export default function LoginPage() {
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
      
      // For now just redirect to a dashboard stub
      window.location.href = '/dashboard';
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-4 relative overflow-hidden bg-[#F5F5F7]">
      {/* Subtle background element */}
      <div className="absolute top-[20%] right-[-10%] w-[40%] h-[60%] rounded-full bg-blue-100 blur-[100px] opacity-50 z-0"></div>
      
      <div className="w-full max-w-sm bg-white/70 backdrop-blur-2xl border border-white/50 shadow-[0_8px_40px_rgb(0,0,0,0.04)] rounded-3xl p-8 z-10 relative">
        <div className="text-center mb-8 mt-2">
          <h1 className="text-3xl font-semibold tracking-tight text-gray-900 mb-2">ERAVAYA</h1>
          <p className="text-gray-500 text-sm">Sign in to your account</p>
        </div>

        {errorMsg && (
          <div className="mb-6 p-4 bg-red-50/50 backdrop-blur-md border border-red-100 text-red-600 text-sm rounded-xl text-center">
            {errorMsg}
          </div>
        )}

        <form onSubmit={handleLogin} className="space-y-4">
          <div className="space-y-1">
            <input required name="school_code" type="text" placeholder="School Code (e.g. ERA01)" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all uppercase" />
          </div>

          <div className="space-y-1">
            <input required name="username" type="text" placeholder="Username" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
          </div>

          <div className="space-y-1 pb-4">
            <input required name="password" type="password" placeholder="Password" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
          </div>

          <button disabled={loading} type="submit" className="w-full bg-[#0071E3] hover:bg-[#0077ED] disabled:bg-[#0071E3]/50 text-white font-medium py-3 rounded-xl transition-all shadow-sm text-sm">
            {loading ? "Signing in..." : "Sign In"}
          </button>
        </form>
      </div>
    </div>
  );
}
