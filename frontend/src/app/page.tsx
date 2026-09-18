"use client";

import { useState } from "react";

export default function SetupPage() {
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");

  const handleSetup = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading(true);
    setErrorMsg("");

    const formData = new FormData(e.currentTarget);
    const data = Object.fromEntries(formData.entries());

    try {
      const res = await fetch("/api/v1/auth/setup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
      });

      if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || "Setup failed");
      }

      setSuccess(true);
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="min-h-screen flex items-center justify-center">
        <div className="w-full max-w-md bg-white/70 backdrop-blur-2xl border border-white/40 shadow-sm rounded-3xl p-10 text-center">
          <div className="w-16 h-16 bg-green-500 rounded-full flex items-center justify-center mx-auto mb-6 shadow-md">
            <svg className="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
            </svg>
          </div>
          <h2 className="text-2xl font-semibold tracking-tight mb-2">Setup Complete</h2>
          <p className="text-gray-500 text-sm mb-8">Your ERP has been initialized. You can now log in securely.</p>
          <button onClick={() => window.location.href = '/login'} className="w-full bg-[#0071E3] hover:bg-[#0077ED] text-white font-medium py-3 rounded-xl transition-all">
            Continue to Login
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center p-4 relative overflow-hidden">
      {/* Subtle background element */}
      <div className="absolute top-[-20%] left-[-10%] w-[50%] h-[50%] rounded-full bg-blue-100 blur-[120px] opacity-60 z-0"></div>
      
      <div className="w-full max-w-md bg-white/60 backdrop-blur-2xl border border-white/50 shadow-[0_8px_40px_rgb(0,0,0,0.04)] rounded-3xl p-8 z-10 relative">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-semibold tracking-tight text-gray-900 mb-2">Welcome</h1>
          <p className="text-gray-500 text-sm">Initialize your ERAVAYA environment.</p>
        </div>

        {errorMsg && (
          <div className="mb-6 p-4 bg-red-50/50 backdrop-blur-md border border-red-100 text-red-600 text-sm rounded-xl text-center">
            {errorMsg}
          </div>
        )}

        <form onSubmit={handleSetup} className="space-y-4">
          <div className="flex gap-4">
            <div className="space-y-1 w-full">
              <label className="text-xs font-medium text-gray-500 ml-1">School Name</label>
              <input required name="school_name" type="text" placeholder="ERAVAYA Academy" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
            </div>
            <div className="space-y-1 w-full">
              <label className="text-xs font-medium text-gray-500 ml-1">Code</label>
              <input required name="school_code" type="text" placeholder="ERA01" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all uppercase" />
            </div>
          </div>

          <div className="flex gap-4 pt-2">
            <div className="space-y-1 w-full">
              <label className="text-xs font-medium text-gray-500 ml-1">Admin First Name</label>
              <input required name="first_name" type="text" placeholder="Abhiram" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
            </div>
            <div className="space-y-1 w-full">
              <label className="text-xs font-medium text-gray-500 ml-1">Last Name</label>
              <input required name="last_name" type="text" placeholder="Admin" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
            </div>
          </div>

          <div className="space-y-1 pt-2">
            <label className="text-xs font-medium text-gray-500 ml-1">Admin Username</label>
            <input required name="admin_username" type="text" placeholder="admin" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
          </div>

          <div className="space-y-1">
            <label className="text-xs font-medium text-gray-500 ml-1">Admin Email</label>
            <input required name="admin_email" type="email" placeholder="admin@eravaya.com" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
          </div>

          <div className="space-y-1 pb-4">
            <label className="text-xs font-medium text-gray-500 ml-1">Secure Password</label>
            <input required name="admin_password" type="password" placeholder="••••••••" className="w-full bg-white/50 border border-gray-200 focus:border-[#0071E3] focus:ring-1 focus:ring-[#0071E3] outline-none rounded-xl px-4 py-3 text-sm transition-all" />
          </div>

          <button disabled={loading} type="submit" className="w-full bg-[#0071E3] hover:bg-[#0077ED] disabled:bg-[#0071E3]/50 text-white font-medium py-3 rounded-xl transition-all shadow-sm text-sm">
            {loading ? "Initializing System..." : "Complete Setup"}
          </button>
        </form>
      </div>
    </div>
  );
}
