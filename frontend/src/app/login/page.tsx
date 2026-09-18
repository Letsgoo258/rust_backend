"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Image from "next/image";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";

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
    <div className="min-h-screen flex w-full bg-white font-sans overflow-hidden">
      
      {/* LEFT COLUMN: Graphic / Brand Side */}
      <div className="hidden lg:flex flex-col relative w-1/2 bg-gradient-to-br from-[#E3F2FD] via-[#E1F5FE] to-[#E0F7FA] border-r border-gray-200">
        {/* Subtle decorative meshes */}
        <div className="absolute inset-0 opacity-40 bg-[url('https://grainy-gradients.vercel.app/noise.svg')] mix-blend-overlay"></div>
        <div className="absolute top-[-20%] left-[-10%] w-[70%] h-[70%] rounded-full bg-gradient-to-r from-blue-300 to-cyan-200 blur-[120px] opacity-60"></div>
        <div className="absolute bottom-[-10%] right-[-10%] w-[60%] h-[60%] rounded-full bg-gradient-to-r from-indigo-200 to-blue-200 blur-[100px] opacity-60"></div>

        <div className="relative z-10 p-12 h-full flex flex-col justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-[#0C152E] rounded-md flex items-center justify-center">
              <span className="text-white font-bold text-sm">E</span>
            </div>
            <span className="font-semibold tracking-tight text-xl text-[#0C152E]">Eravaya</span>
          </div>

          <div className="max-w-md pb-10">
            <h2 className="text-[32px] font-bold text-[#0C152E] leading-tight mb-8">
              Join leading schools that trust Eravaya to Supercharge their Administration
            </h2>
            <div className="flex flex-wrap gap-x-8 gap-y-4">
              <div className="flex items-center text-sm font-medium text-[#0C152E]/80">
                <svg className="w-4 h-4 mr-2 text-[#3366FF]" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="3" d="M12 4v16m8-8H4"></path></svg>
                Smart Academics
              </div>
              <div className="flex items-center text-sm font-medium text-[#0C152E]/80">
                <svg className="w-4 h-4 mr-2 text-[#3366FF]" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="3" d="M12 4v16m8-8H4"></path></svg>
                Easy Integration
              </div>
              <div className="flex items-center text-sm font-medium text-[#0C152E]/80">
                <svg className="w-4 h-4 mr-2 text-[#3366FF]" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="3" d="M12 4v16m8-8H4"></path></svg>
                Powerful Dashboard
              </div>
            </div>
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
            <p className="text-gray-500 text-sm mb-2">Welcome to <span className="font-semibold text-gray-900">Eravaya</span></p>
            <h1 className="text-3xl font-bold tracking-tight text-gray-900 leading-tight">
              Get started with your email or username
            </h1>
          </div>

          {errorMsg && (
            <div className="mb-6 p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg">
              {errorMsg}
            </div>
          )}

          <form onSubmit={handleLogin} className="space-y-4">
            <Input 
              required 
              name="username" 
              type="text" 
              placeholder="Enter your email or username" 
            />

            <Input 
              required 
              name="password" 
              type="password" 
              placeholder="Password" 
            />

            <Button type="submit" loading={loading} loadingText="Continuing..." fullWidth className="mt-2">
              Continue
            </Button>
          </form>

          <div className="flex items-center my-6">
            <div className="flex-grow h-px bg-gray-200"></div>
            <span className="px-4 text-xs text-gray-400 font-medium lowercase">or</span>
            <div className="flex-grow h-px bg-gray-200"></div>
          </div>

          <button disabled type="button" className="w-full bg-gray-50 border border-gray-200 text-gray-400 font-semibold py-3 rounded-lg text-[15px] shadow-sm flex items-center justify-center gap-2 cursor-not-allowed grayscale opacity-70">
            <svg className="w-5 h-5" viewBox="0 0 24 24"><path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/><path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/><path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/><path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/><path fill="none" d="M1 1h22v22H1z"/></svg>
            Continue with Google (Coming Soon)
          </button>

          <div className="mt-10">
            <p className="text-[11px] text-gray-500 leading-relaxed mb-4">
              By continuing you agree to our <a href="#" className="text-[#3366FF] hover:underline">privacy policy</a> &amp; <a href="#" className="text-[#3366FF] hover:underline">terms of use</a>.
              <br/>*Limited period offer, <a href="#" className="text-[#3366FF] hover:underline">terms and conditions</a> apply.
            </p>

            <div className="bg-gray-50 rounded-xl p-4">
              <p className="text-xs font-semibold text-gray-900 mb-1">Helping Schools with Eravaya Solutions?</p>
              <a href="#" className="text-xs text-[#3366FF] hover:underline font-medium flex items-center gap-1">
                Become Eravaya Partner <span aria-hidden="true">&rarr;</span>
              </a>
            </div>
          </div>

        </div>
      </div>

    </div>
  );
}
