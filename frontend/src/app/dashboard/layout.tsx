"use client";

import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import Link from "next/link";
import { Input } from "@/components/ui/Input";
import { Button } from "@/components/ui/Button";

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const [user, setUser] = useState<{ display_name?: string; username: string; school_id?: string; user_type?: string; email_verified?: boolean; phone_verified?: boolean; school_status?: string } | null>(null);
  
  const [verifyingEmail, setVerifyingEmail] = useState(false);
  const [verifyingPhone, setVerifyingPhone] = useState(false);
  const [showPhoneModal, setShowPhoneModal] = useState(false);
  const [otp, setOtp] = useState("");
  const [phoneError, setPhoneError] = useState("");

  const handleVerifyEmail = async () => {
    setVerifyingEmail(true);
    try {
      const res = await fetch("/api/v1/auth/verify/email", { method: "POST" });
      if (res.ok) {
        setUser(prev => prev ? { ...prev, email_verified: true } : null);
      }
    } finally {
      setVerifyingEmail(false);
    }
  };

  
  const handleSetupSchool = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const payload = Object.fromEntries(formData.entries());
    
    try {
      const res = await fetch("/api/v1/auth/schools/setup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      if (res.ok) {
        setUser(prev => prev ? { ...prev, school_status: "ACTIVE" } : null);
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleVerifyPhone = async (e: React.FormEvent) => {
    e.preventDefault();
    setVerifyingPhone(true);
    setPhoneError("");
    try {
      const res = await fetch("/api/v1/auth/verify/phone", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ otp })
      });
      if (res.ok) {
        setUser(prev => prev ? { ...prev, phone_verified: true } : null);
        setShowPhoneModal(false);
      } else {
        setPhoneError("Invalid OTP. Hint: 123456");
      }
    } finally {
      setVerifyingPhone(false);
    }
  };
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch("/api/v1/auth/me")
      .then((res) => {
        if (res.ok) return res.json();
        throw new Error("Not logged in");
      })
      .then((data) => {
        setUser(data);
        setLoading(false);
      })
      .catch(() => {
        router.push("/login");
      });
  }, [router]);

  if (loading) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-[#F4F5F7]">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-[#3366FF]"></div>
      </div>
    );
  }

  const isSuperAdmin = user?.user_type === "SUPER_ADMIN";

  const topNavItems = [
    { name: "Eravaya Home", path: "/dashboard", icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6", active: pathname === "/dashboard" || (!pathname.includes("/super-admin") && !pathname.includes("/analytics") && !pathname.includes("/reports") && !pathname.includes("/observe") && !pathname.includes("/admissions") && !pathname.includes("/permissions")) },
    ...(isSuperAdmin ? [{ name: "Platform Admin", path: "/dashboard/super-admin", icon: "M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4", active: pathname.includes("/super-admin") }] : []),
    { name: "Analytics", path: "/dashboard/analytics", icon: "M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z", active: pathname.includes("/analytics") },
    { name: "Reports", path: "/dashboard/reports", icon: "M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z", active: pathname.includes("/reports") },
    { name: "Observe", path: "/dashboard/observe", icon: "M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z", active: pathname.includes("/observe") },
    { name: "Admissions", path: "/dashboard/admissions", icon: "M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z", active: pathname.includes("/admissions") },
    { name: "Permissions", path: "/dashboard/permissions", icon: "M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z", active: pathname.includes("/permissions") },
  ];

  const sideNavItems = isSuperAdmin ? [
    { section: "PLATFORM", items: [
      { name: "Overview", path: "/dashboard/super-admin", icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" },
      { name: "School Tenants", path: "/dashboard/super-admin/schools/new", icon: "M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4" }
    ]}
  ] : [
    { section: "HOME", items: [
      { name: "Overview", path: "/dashboard", icon: "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6" },
    ]},
    { section: "CORE MODULES", items: [
      { name: "Students", path: "/dashboard/students", icon: "M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" },
      { name: "Academics", path: "/dashboard/academics", icon: "M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" },
      { name: "Settings", path: "/dashboard/settings", icon: "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065zM15 12a3 3 0 11-6 0 3 3 0 016 0z" },
    ]}
  ];

  return (
    <div className="h-screen bg-[#F4F5F7] flex flex-col font-sans overflow-hidden">
      {/* Top Navbar */}
      <header className="h-14 bg-[#0C152E] flex items-center justify-between px-4 z-20 shrink-0">
        <div className="flex items-center gap-8 h-full">
          {/* Logo */}
          <Link href="/dashboard" className="flex items-center gap-2">
            <div className="w-7 h-7 bg-white rounded-md flex items-center justify-center">
              <span className="text-[#0C152E] font-bold text-sm">E</span>
            </div>
            <span className="font-semibold text-white tracking-tight text-lg">Eravaya <span className="text-[10px] font-normal text-white/60 align-top uppercase tracking-wider ml-1">{isSuperAdmin ? "Platform" : "Standard"}</span></span>
          </Link>

          {/* Top Nav Links */}
          <nav className="hidden md:flex items-center h-full gap-2">
            {topNavItems.map(item => (
              <Link 
                key={item.path} 
                href={item.path}
                className={`flex items-center gap-2 h-full px-4 text-sm font-semibold border-b-2 transition-colors ${item.active ? 'border-[#3366FF] text-white' : 'border-transparent text-gray-400 hover:text-gray-200'}`}
              >
                {item.icon && (
                  <svg className="w-4 h-4 opacity-75" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth="2">
                    <path strokeLinecap="round" strokeLinejoin="round" d={item.icon}></path>
                  </svg>
                )}
                {item.name}
              </Link>
            ))}
          </nav>
        </div>

        {/* Right side actions */}
        <div className="flex items-center gap-4">
          <div className="hidden lg:flex items-center bg-white/10 rounded-lg px-3 py-1.5 border border-white/10 w-64 focus-within:ring-1 focus-within:ring-[#3366FF] focus-within:bg-white/15 transition-all">
            <svg className="w-4 h-4 text-gray-400 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path></svg>
            <input type="text" placeholder="Search students, settings..." className="bg-transparent border-none outline-none text-sm text-white placeholder:text-gray-400 w-full" />
          </div>
          
          <button className="w-8 h-8 rounded-full bg-[#1C2640] border border-white/10 flex items-center justify-center text-white/80 hover:text-white transition-colors">
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"></path></svg>
          </button>
          
          <div className="w-8 h-8 rounded-full bg-gradient-to-tr from-[#3366FF] to-[#00C2FF] flex items-center justify-center text-white font-medium text-xs shadow-sm cursor-pointer border border-white/10 relative group">
            {(user?.display_name || user?.username || "U").charAt(0).toUpperCase()}
            
            {/* Dropdown menu */}
            <div className="absolute right-0 top-10 w-48 bg-white rounded-xl shadow-lg border border-gray-100 opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50">
              <div className="px-4 py-3 border-b border-gray-100">
                <p className="text-sm font-medium text-gray-900 truncate">{user?.display_name || user?.username}</p>
                <p className="text-xs text-gray-500 truncate capitalize">{user?.user_type?.replace('_', ' ').toLowerCase() || 'User'}</p>
              </div>
              <div className="p-2">
                <button 
                  onClick={() => {
                    fetch("/api/v1/auth/logout", { method: "POST" }).then(() => router.push("/login"));
                  }}
                  className="w-full text-left px-3 py-2 text-sm text-red-600 hover:bg-red-50 rounded-lg transition-colors"
                >
                  Sign Out
                </button>
              </div>
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Left Sidebar */}
        <aside className="w-[240px] bg-[#F9FAFB] border-r border-gray-200 flex flex-col shrink-0 overflow-y-auto hidden md:flex">
          <div className="p-4 flex-1">
            {sideNavItems.map((section, idx) => (
              <div key={idx} className="mb-6">
                <h3 className="px-3 text-[11px] font-bold text-gray-400 uppercase tracking-wider mb-2">{section.section}</h3>
                <div className="space-y-1">
                  {section.items.map(item => {
                    const isActive = pathname === item.path || (item.path !== '/dashboard' && pathname.startsWith(item.path));
                    return (
                      <Link 
                        key={item.path} 
                        href={item.path}
                        className={`flex items-center gap-3 px-3 py-2 rounded-lg text-[13px] font-medium transition-all ${
                          isActive 
                            ? 'bg-[#EAEFFD] text-[#3366FF]' 
                            : 'text-gray-600 hover:bg-gray-100 hover:text-gray-900'
                        }`}
                      >
                        <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={isActive ? "2.5" : "2"} d={item.icon}></path>
                        </svg>
                        {item.name}
                      </Link>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>

          <div className="p-4 border-t border-gray-200">
            <div className="flex items-center justify-between text-xs text-gray-500 font-medium px-2 py-1">
              <span>Test Mode</span>
              <div className="w-8 h-4 bg-gray-200 rounded-full relative cursor-pointer">
                <div className="w-3 h-3 bg-white rounded-full absolute top-[2px] left-[2px] shadow-sm"></div>
              </div>
            </div>
          </div>
        </aside>

        {/* Main Content */}
        <main className="flex-1 overflow-auto bg-[#F4F5F7] p-6 lg:p-8 relative">
          <div className="max-w-6xl mx-auto">
            
            {user && (!user.email_verified || !user.phone_verified) && (
              <div className="mb-8 bg-yellow-50 border-l-4 border-yellow-400 p-4 rounded-r-lg shadow-sm">
                <div className="flex items-start">
                  <div className="flex-shrink-0">
                    <svg className="h-5 w-5 text-yellow-400" viewBox="0 0 20 20" fill="currentColor">
                      <path fillRule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
                    </svg>
                  </div>
                  <div className="ml-3">
                    <h3 className="text-sm font-medium text-yellow-800">
                      Action Required: Verify your identity
                    </h3>
                    <div className="mt-2 text-sm text-yellow-700">
                      <p>
                        {!user.email_verified 
                          ? "Please verify your email address to secure your account."
                          : "Please verify your phone number via WhatsApp to complete onboarding."}
                      </p>
                    </div>
                    <div className="mt-4">
                      {!user.email_verified ? (
                        <button
                          onClick={handleVerifyEmail}
                          disabled={verifyingEmail}
                          className="bg-yellow-100 text-yellow-800 hover:bg-yellow-200 px-3 py-1.5 rounded-md text-sm font-medium transition-colors"
                        >
                          {verifyingEmail ? "Sending..." : "Send Verification Email"}
                        </button>
                      ) : (
                        <button
                          onClick={() => setShowPhoneModal(true)}
                          className="bg-yellow-100 text-yellow-800 hover:bg-yellow-200 px-3 py-1.5 rounded-md text-sm font-medium transition-colors"
                        >
                          Verify Phone Number
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            )}

            {user?.school_status === 'PENDING_SETUP' && user?.email_verified && user?.phone_verified ? (
              <div className="bg-white rounded-xl shadow-sm border border-gray-200/60 p-8 max-w-3xl mx-auto">
                <h2 className="text-xl font-bold text-gray-900 mb-2">Complete School Profile</h2>
                <p className="text-sm text-gray-500 mb-8">Before you can access your dashboard, please complete your school's profile.</p>
                <form onSubmit={handleSetupSchool} className="space-y-6">
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <Input required name="short_name" type="text" label="Short Name (e.g. GWH)" />
                    <Input name="website" type="url" label="Website URL" placeholder="https://" />
                  </div>
                  
                  <h3 className="text-md font-medium text-gray-900 border-b pb-2 pt-4">Location Details</h3>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                    <div className="md:col-span-2">
                      <Input required name="address_line_1" type="text" label="Address Line 1" />
                    </div>
                    <div className="md:col-span-2">
                      <Input name="address_line_2" type="text" label="Address Line 2 (Optional)" />
                    </div>
                    <Input required name="city" type="text" label="City" />
                    <Input required name="district" type="text" label="District" />
                    <Input required name="state" type="text" label="State/Province" />
                    <Input required name="country" type="text" label="Country" />
                    <Input required name="postal_code" type="text" label="Postal/Zip Code" />
                    <Input required name="timezone" type="text" label="Timezone" defaultValue="UTC" />
                  </div>
                  
                  <div className="pt-6">
                    <Button type="submit" className="w-full">Save & Activate School</Button>
                  </div>
                </form>
              </div>
            ) : children}
          </div>
          
          {/* Phone Verification Modal */}
          {showPhoneModal && (
            <div className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 flex items-center justify-center p-4">
              <div className="bg-white rounded-xl shadow-xl w-full max-w-md p-6">
                <h2 className="text-lg font-bold text-gray-900 mb-2">Verify Phone Number</h2>
                <p className="text-sm text-gray-500 mb-6">Enter the 6-digit WhatsApp OTP sent to your registered number.</p>
                
                <form onSubmit={handleVerifyPhone}>
                  <div className="space-y-4">
                    <div>
                      <input 
                        type="text" 
                        value={otp}
                        onChange={(e) => setOtp(e.target.value)}
                        placeholder="123456" 
                        maxLength={6}
                        className="w-full text-center text-2xl tracking-[0.5em] font-mono bg-gray-50 border border-gray-300 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-4 py-3"
                      />
                      {phoneError && <p className="text-xs text-red-500 mt-2 text-center">{phoneError}</p>}
                    </div>
                    <div className="flex gap-3">
                      <button 
                        type="button" 
                        onClick={() => setShowPhoneModal(false)}
                        className="flex-1 bg-gray-100 hover:bg-gray-200 text-gray-700 font-medium py-2.5 rounded-lg text-sm transition-colors"
                      >
                        Cancel
                      </button>
                      <button 
                        type="submit"
                        disabled={verifyingPhone || otp.length < 6}
                        className="flex-1 bg-[#3366FF] hover:bg-[#2B57D9] disabled:opacity-50 text-white font-medium py-2.5 rounded-lg text-sm transition-colors"
                      >
                        {verifyingPhone ? "Verifying..." : "Verify OTP"}
                      </button>
                    </div>
                  </div>
                </form>
              </div>
            </div>
          )}

        </main>
      </div>
    </div>
  );
}
