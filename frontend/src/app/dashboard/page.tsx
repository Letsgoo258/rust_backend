"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

interface User {
  username: string;
  display_name?: string;
  email_verified: boolean;
  phone_verified: boolean;
  school_status: string;
}

export default function DashboardOverview() {
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch("/api/v1/auth/me")
      .then(res => {
        if (!res.ok) throw new Error("Not logged in");
        return res.json();
      })
      .then(data => {
        setUser(data.user);
        setLoading(false);
      })
      .catch(() => {
        router.push("/login");
      });
  }, [router]);

  if (loading) return null;

  const needsOnboarding = !user?.email_verified || !user?.phone_verified || user?.school_status === "PENDING_SETUP";

  return (
    <div className="p-8 md:p-12 max-w-6xl mx-auto animate-in fade-in slide-in-from-bottom-4 duration-500">
      <header className="mb-10">
        <h1 className="text-3xl font-semibold text-gray-900 tracking-tight mb-2">
          {needsOnboarding ? "Welcome to Eravaya!" : `Good morning, ${user?.display_name || user?.username}`}
        </h1>
        <p className="text-gray-500 text-lg">
          {needsOnboarding ? "Let's get your school system up and running." : "Here is what's happening at your school today."}
        </p>
      </header>

      {needsOnboarding && (
        <div className="mb-10 w-full bg-gradient-to-r from-blue-50 to-[#EAEFFD] rounded-3xl border border-blue-100/50 p-1 flex flex-col md:flex-row overflow-hidden shadow-sm">
          {/* Banner Left / Content */}
          <div className="p-8 md:p-12 flex-1 flex flex-col justify-center">
            <h2 className="text-2xl md:text-4xl font-bold text-[#0C152E] tracking-tight mb-4 leading-tight">
              Everything beyond management, <br/>built into one platform
            </h2>
            <p className="text-[#3366FF] font-medium mb-8">Set up Admins, Students, Classes, Timetables & Rules in minutes.</p>
            
            <div className="flex flex-col gap-3 max-w-sm mb-10">
              <div className="flex items-center gap-3 bg-white/60 px-4 py-2 rounded-full border border-white">
                <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"></path></svg>
                <span className="text-sm font-medium text-gray-700">For schools of any size</span>
              </div>
              <div className="flex items-center gap-3 bg-white/60 px-4 py-2 rounded-full border border-white">
                <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"></path></svg>
                <span className="text-sm font-medium text-gray-700">Step-by-step onboarding</span>
              </div>
              <div className="flex items-center gap-3 bg-white/60 px-4 py-2 rounded-full border border-white">
                <svg className="w-5 h-5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M5 13l4 4L19 7"></path></svg>
                <span className="text-sm font-medium text-gray-700">Included with Eravaya</span>
              </div>
            </div>
            
          </div>
          
          {/* Banner Right / Actions */}
          <div className="bg-white/40 md:w-1/3 border-l border-white/50 p-8 md:p-12 flex flex-col justify-center items-start backdrop-blur-sm">
            <h3 className="font-semibold text-gray-900 mb-2">Next Step</h3>
            <p className="text-sm text-gray-600 mb-6">
              {!user?.email_verified || !user?.phone_verified 
                ? "Verify your contact details in the top banner to unlock your school dashboard." 
                : "Fill out your official school profile to unlock all core modules."}
            </p>
            <Button 
              onClick={() => {
                if (!user?.email_verified || !user?.phone_verified) {
                  window.scrollTo({ top: 0, behavior: 'smooth' });
                  alert("Please use the verification banner at the top of the page first!");
                } else {
                  router.push("/dashboard/schools/setup");
                }
              }}
              className="bg-[#3366FF] hover:bg-blue-600 text-white shadow-md shadow-blue-500/20 py-6 px-8 text-base rounded-2xl w-full"
            >
              {!user?.email_verified || !user?.phone_verified ? "Verify Account" : "Complete School Profile"}
            </Button>
          </div>
        </div>
      )}

      {!needsOnboarding && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm hover:shadow-md transition-shadow">
            <h3 className="text-sm font-medium text-gray-500 mb-1">Total Students</h3>
            <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
          </div>
          <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm hover:shadow-md transition-shadow">
            <h3 className="text-sm font-medium text-gray-500 mb-1">Active Classes</h3>
            <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
          </div>
          <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm hover:shadow-md transition-shadow">
            <h3 className="text-sm font-medium text-gray-500 mb-1">Pending Enrollments</h3>
            <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
          </div>
        </div>
      )}
    </div>
  );
}
