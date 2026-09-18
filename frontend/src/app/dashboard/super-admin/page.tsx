"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { Button } from "@/components/ui/Button";

interface School {
  id: string;
  name: string;
  code: string;
  status: string;
  created_at: string;
}

export default function SuperAdminDashboard() {
  const [schools, setSchools] = useState<School[]>([]);
  const [loading, setLoading] = useState(true);

  // In a real app, this would fetch from an API
  useEffect(() => {
    // Mock data for the UI
    setTimeout(() => {
      setSchools([
        { id: "1", name: "Eravaya HQ", code: "HQ01", status: "ACTIVE", created_at: "2026-09-18" },
        { id: "2", name: "Greenwood High", code: "GWH01", status: "ACTIVE", created_at: "2026-09-19" },
      ]);
      setLoading(false);
    }, 500);
  }, []);

  return (
    <div className="space-y-8 animate-in fade-in duration-500">
      
      {/* Overview Header Area */}
      <div className="flex items-center justify-between">
        <h1 className="text-xl font-bold text-gray-900 flex items-center gap-2">
          Platform Overview <span className="text-xs font-medium text-gray-400 font-mono tracking-wider">Just now <svg className="w-3 h-3 inline pb-px" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"></path></svg></span>
        </h1>
        <Link href="/dashboard/super-admin/schools/new" className="text-sm font-medium text-[#3366FF] hover:underline flex items-center gap-1">
          Add new school tenant <span aria-hidden="true">&rarr;</span>
        </Link>
      </div>

      {/* Main Metric Card */}
      <div className="bg-white rounded-xl shadow-sm border border-gray-200/60 overflow-hidden flex flex-col lg:flex-row">
        
        {/* Left Side: Stats */}
        <div className="flex-1 p-6 lg:p-8 flex flex-col sm:flex-row gap-8 lg:gap-16 border-b lg:border-b-0 lg:border-r border-gray-100">
          <div>
            <div className="w-8 h-8 rounded bg-blue-50 flex items-center justify-center mb-3">
              <svg className="w-4 h-4 text-[#3366FF]" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1 flex items-center gap-1">
              Active Tenants <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 5l7 7-7 7"></path></svg>
            </p>
            <p className="text-xs text-gray-500 mb-4">Updated on Sep 19, 2026</p>
            <div className="flex items-baseline gap-1">
              <span className="text-2xl font-bold text-gray-900">{schools.length}</span>
            </div>
            <p className="text-xs font-semibold text-gray-900 underline underline-offset-4 decoration-gray-300 decoration-2 mt-4 cursor-pointer hover:decoration-gray-400">View tenant breakup</p>
          </div>

          <div>
            <div className="w-8 h-8 rounded bg-green-50 flex items-center justify-center mb-3">
              <svg className="w-4 h-4 text-green-600" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1">System Health</p>
            <p className="text-xs text-gray-500 mb-4">All services operational</p>
          </div>

          <div>
            <div className="w-8 h-8 rounded bg-gray-50 flex items-center justify-center mb-3 border border-gray-100">
              <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"></path></svg>
            </div>
            <p className="text-sm font-medium text-gray-800 mb-1">Next Maintenance</p>
            <p className="text-xs text-gray-500 mb-4">No upcoming maintenance</p>
            <p className="text-xs text-orange-500 font-medium flex items-center gap-1 mt-4">
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path></svg>
              No downtime on 20th Sep (Sunday)
            </p>
          </div>
        </div>

        {/* Right Side: Action/Balance */}
        <div className="p-6 lg:p-8 bg-[#FAFAFA] min-w-[280px]">
          <p className="text-xs font-bold text-gray-600 border-b border-gray-300 pb-1 mb-4 inline-block border-dashed">Total Users</p>
          <div className="flex items-baseline mb-6">
            <span className="text-3xl font-bold text-gray-900 tracking-tight">1,204</span>
          </div>
          
          <p className="text-xs font-semibold text-gray-800 mb-2">Need a system report?</p>
          <button className="w-full bg-white border border-gray-200 text-gray-400 font-medium py-2 rounded-lg text-sm shadow-sm flex items-center justify-center gap-2 cursor-not-allowed">
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path></svg>
            Generate Report
          </button>
        </div>
      </div>

      {/* Promotional Banner (Key Updates) */}
      <div>
        <h2 className="text-lg font-bold text-gray-900 mb-4">Key Updates - You're all caught up!</h2>
        <div className="w-full bg-gradient-to-r from-[#EBF2FA] to-[#E3EAF7] rounded-xl overflow-hidden flex flex-col md:flex-row items-center p-8 relative">
          <div className="flex-1 relative z-10">
            <div className="flex items-center gap-2 mb-3">
              <span className="text-[#3366FF] font-bold italic tracking-tight">Eravaya</span>
              <span className="bg-[#3366FF] text-white text-[10px] font-bold px-1.5 py-0.5 rounded uppercase tracking-wider">Payroll</span>
            </div>
            <h3 className="text-2xl font-bold text-[#0C152E] leading-tight mb-3">
              Everything beyond payroll,<br/>built into payroll
            </h3>
            <p className="text-sm text-gray-600 max-w-sm mb-6">
              Health benefits, emergency funds, tax-filing and more for schools of any size
            </p>
          </div>
          
          <div className="flex-1 flex justify-center md:justify-end gap-6 relative z-10 w-full md:w-auto">
            <div className="space-y-2 hidden lg:block">
              <div className="bg-[#D1F2EB] text-[#0C152E] text-xs font-medium px-4 py-1.5 rounded-full text-center">For schools of any size</div>
              <div className="bg-[#D1F2EB] text-[#0C152E] text-xs font-medium px-4 py-1.5 rounded-full text-center">Starting from day one</div>
              <div className="bg-[#D1F2EB] text-[#0C152E] text-xs font-medium px-4 py-1.5 rounded-full text-center">Included with Eravaya Payroll</div>
            </div>
            <div className="flex items-center">
              <button className="bg-[#3366FF] hover:bg-[#2B57D9] text-white font-bold py-3 px-6 rounded-lg shadow-sm transition-all text-sm w-full md:w-auto">
                Sign Up for Payroll Now
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Tenants Table */}
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-gray-900">School Tenants Overview</h2>
          <div className="flex gap-2">
            <button className="bg-white border border-gray-200 text-gray-700 text-sm font-medium px-3 py-1.5 rounded-md flex items-center gap-2 shadow-sm">
              <svg className="w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"></path></svg>
              This Month <svg className="w-3 h-3 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 9l-7 7-7-7"></path></svg>
            </button>
          </div>
        </div>
        
        <div className="bg-white rounded-xl shadow-sm border border-gray-200/60 overflow-hidden">
          <div className="grid grid-cols-3 border-b border-gray-200">
            <div className="p-4 border-r border-gray-200">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">New Tenants <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">2</p>
            </div>
            <div className="p-4 border-r border-gray-200">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">Cancellations <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">0</p>
            </div>
            <div className="p-4">
              <p className="text-xs font-medium text-gray-500 mb-1 flex items-center gap-1">Total Revenue <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg></p>
              <p className="text-lg font-bold text-gray-900">$0.00</p>
            </div>
          </div>
          
          <div className="p-6">
            {loading ? (
              <div className="animate-pulse space-y-4">
                <div className="h-4 bg-gray-200 rounded w-1/4"></div>
                <div className="h-4 bg-gray-200 rounded w-1/2"></div>
              </div>
            ) : (
              <table className="w-full text-sm text-left">
                <thead className="text-xs text-gray-500 bg-gray-50 border-b border-gray-200">
                  <tr>
                    <th className="px-4 py-3 font-medium">School Name</th>
                    <th className="px-4 py-3 font-medium">Code</th>
                    <th className="px-4 py-3 font-medium">Status</th>
                    <th className="px-4 py-3 font-medium text-right">Created</th>
                  </tr>
                </thead>
                <tbody>
                  {schools.map(s => (
                    <tr key={s.id} className="border-b border-gray-100 last:border-0 hover:bg-gray-50 transition-colors">
                      <td className="px-4 py-4 font-medium text-gray-900">{s.name}</td>
                      <td className="px-4 py-4 text-gray-500 font-mono text-xs">{s.code}</td>
                      <td className="px-4 py-4">
                        <span className="bg-green-100 text-green-700 text-[10px] font-bold px-2 py-1 rounded-full uppercase tracking-wider">{s.status}</span>
                      </td>
                      <td className="px-4 py-4 text-right text-gray-500">{s.created_at}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
      
    </div>
  );
}
