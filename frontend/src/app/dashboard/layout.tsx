"use client";

import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import Link from "next/link";

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const [user, setUser] = useState<{ display_name?: string; username: string; school_id?: string; user_type?: string } | null>(null);
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
    { name: "Eravaya Home", path: "/dashboard", active: !pathname.includes("/super-admin") },
    ...(isSuperAdmin ? [{ name: "Platform Admin", path: "/dashboard/super-admin", active: pathname.includes("/super-admin") }] : []),
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
                className={`flex items-center h-full px-3 text-sm font-medium border-b-2 transition-colors ${item.active ? 'border-[#3366FF] text-white' : 'border-transparent text-gray-400 hover:text-white'}`}
              >
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
        <main className="flex-1 overflow-auto bg-[#F4F5F7] p-6 lg:p-8">
          <div className="max-w-6xl mx-auto">
            {children}
          </div>
        </main>
      </div>
    </div>
  );
}
