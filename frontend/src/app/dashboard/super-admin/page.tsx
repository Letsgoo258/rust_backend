"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

interface School {
  id: string;
  name: string;
  code: string;
  status: string;
  created_at: string;
}

interface SuperAdminStats {
  schools: School[];
}

export default function SuperAdminPage() {
  const router = useRouter();
  const [stats, setStats] = useState<SuperAdminStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [impersonating, setImpersonating] = useState<string | null>(null);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deletingSchoolId, setDeletingSchoolId] = useState<string | null>(null);
  const [otp, setOtp] = useState("");
  const [deleteError, setDeleteError] = useState("");
  const [deleteLoading, setDeleteLoading] = useState(false);

  useEffect(() => {
    fetch("/api/v1/auth/super-admin/stats")
      .then(res => {
        if (!res.ok) throw new Error("API Error");
        return res.json();
      })
      .then(data => {
        setStats(data);
        setLoading(false);
      })
      .catch(err => {
        console.error(err);
        setLoading(false);
      });
  }, []);

  const handleToggleStatus = async (schoolId: string, currentStatus: string) => {
    try {
      const newStatus = currentStatus === "ACTIVE" ? "INACTIVE" : "ACTIVE";
      const res = await fetch(`/api/v1/auth/super-admin/schools/${schoolId}/status`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ status: newStatus }),
      });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Failed to update status";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      
      setStats(prev => {
        if (!prev) return prev;
        return {
          ...prev,
          schools: prev.schools.map(s => s.id === schoolId ? { ...s, status: newStatus } : s)
        };
      });
    } catch (err) {
      console.error(err);
      alert("Failed to update status");
    }
  };

  const startDelete = async (schoolId: string) => {
    try {
      setDeletingSchoolId(schoolId);
      setDeleteModalOpen(true);
      setOtp("");
      setDeleteError("");
      const res = await fetch(`/api/v1/auth/super-admin/schools/${schoolId}/request-delete-otp`, {
        method: "POST"
      });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Failed to request OTP";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
    } catch (err) {
      console.error(err);
      setDeleteError("Failed to request OTP. Ensure backend is running.");
    }
  };

  const confirmDelete = async () => {
    if (!deletingSchoolId || !otp) return;
    setDeleteLoading(true);
    setDeleteError("");
    try {
      const res = await fetch(`/api/v1/auth/super-admin/schools/${deletingSchoolId}/delete`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ otp }),
      });
      
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Invalid OTP or failure";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      
      setStats(prev => {
        if (!prev) return prev;
        return {
          ...prev,
          schools: prev.schools.filter(s => s.id !== deletingSchoolId)
        };
      });
      setDeleteModalOpen(false);
    } catch (err: any) {
      console.error(err);
      setDeleteError(err.message || "Failed to delete");
    } finally {
      setDeleteLoading(false);
    }
  };

  const handleImpersonate = async (schoolId: string) => {
    setImpersonating(schoolId);
    try {
      const res = await fetch("/api/v1/auth/switch-tenant", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ school_id: schoolId })
      });
      if (res.ok) {
        window.location.href = "/dashboard";
      } else {
        alert("Failed to switch tenant.");
        setImpersonating(null);
      }
    } catch (err) {
      console.error(err);
      setImpersonating(null);
    }
  };

  return (
    <div className="max-w-6xl mx-auto py-8 px-4">
      <div className="flex justify-between items-center mb-8">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 mb-1">Super Admin Overview</h1>
          <p className="text-sm text-gray-500">Manage school tenants and system access.</p>
        </div>
        <Button onClick={() => router.push("/dashboard/super-admin/schools/new")}>
          + Add New School
        </Button>
      </div>

      <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
        <div className="px-6 py-4 border-b border-gray-200 flex justify-between items-center bg-gray-50/50">
          <h2 className="text-sm font-bold text-gray-700 uppercase tracking-wider">Active School Tenants</h2>
        </div>
        <div className="p-0">
          {loading ? (
            <div className="p-6 text-sm text-gray-500 animate-pulse">Loading tenants...</div>
          ) : (
            <table className="w-full text-sm text-left">
              <thead className="text-xs text-gray-500 bg-gray-50 border-b border-gray-200">
                <tr>
                  <th className="px-6 py-3 font-medium">School Name</th>
                  <th className="px-6 py-3 font-medium">Code</th>
                  <th className="px-6 py-3 font-medium">Status</th>
                  <th className="px-6 py-3 font-medium">Created</th>\n                  <th className="px-6 py-3 font-medium text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {stats?.schools?.map(s => (
                  <tr 
                    key={s.id} 
                    onClick={() => handleImpersonate(s.id)}
                    className="group cursor-pointer hover:bg-gray-50 transition-colors"
                  >
                    <td className="px-6 py-4 font-medium text-gray-900 flex items-center gap-3">
                      <div className="w-8 h-8 rounded-md bg-[#0C152E] flex items-center justify-center text-white font-bold text-xs">
                        {s.name.substring(0,2).toUpperCase()}
                      </div>
                      <div>
                        {s.name}
                        {impersonating === s.id && <span className="ml-2 text-xs text-blue-600 font-semibold animate-pulse">Switching...</span>}
                      </div>
                    </td>
                    <td className="px-6 py-4 text-gray-500 font-mono text-xs">{s.code}</td>
                    <td className="px-6 py-4">
                      <span className={`text-[10px] font-bold px-2.5 py-1 rounded-full uppercase tracking-wider ${
                        s.status === 'ACTIVE' ? 'bg-green-100 text-green-700' : 
                        s.status === 'PENDING_SETUP' ? 'bg-orange-100 text-orange-700' :
                        'bg-red-100 text-red-700'
                      }`}>
                        {s.status}
                      </span>
                    </td>
                    <td className="px-6 py-4 text-gray-500">
                      {new Date(s.created_at).toLocaleDateString()}
                    </td>
                    <td className="px-6 py-4 text-right font-medium space-x-3">
                      <button 
                        onClick={(e) => { e.stopPropagation(); handleToggleStatus(s.id, s.status); }}
                        className="text-gray-500 hover:text-gray-900 bg-gray-100 hover:bg-gray-200 px-3 py-1.5 rounded-md text-xs transition-colors"
                      >
                        {s.status === "ACTIVE" ? "Deactivate" : "Activate"}
                      </button>
                      <button 
                        onClick={(e) => { e.stopPropagation(); startDelete(s.id); }}
                        className="text-red-600 hover:text-white bg-red-50 hover:bg-red-600 px-3 py-1.5 rounded-md text-xs transition-colors"
                      >
                        Delete
                      </button>
                    </td>
                  </tr>
                ))}
                {stats?.schools?.length === 0 && (
                  <tr>
                    <td colSpan={5} className="px-6 py-8 text-center text-gray-500">No schools found. Create one to get started.</td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </div>
      </div>

      {deleteModalOpen && (
        <div className="fixed inset-0 z-50 overflow-y-auto" aria-labelledby="modal-title" role="dialog" aria-modal="true">
          <div className="flex items-end justify-center min-h-screen pt-4 px-4 pb-20 text-center sm:block sm:p-0">
            <div className="fixed inset-0 bg-gray-500 bg-opacity-75 transition-opacity" aria-hidden="true" onClick={() => setDeleteModalOpen(false)}></div>
            <span className="hidden sm:inline-block sm:align-middle sm:h-screen" aria-hidden="true">&#8203;</span>
            <div className="inline-block align-bottom bg-white rounded-lg text-left overflow-hidden shadow-xl transform transition-all sm:my-8 sm:align-middle sm:max-w-lg sm:w-full">
              <div className="bg-white px-4 pt-5 pb-4 sm:p-6 sm:pb-4">
                <div className="sm:flex sm:items-start">
                  <div className="mx-auto flex-shrink-0 flex items-center justify-center h-12 w-12 rounded-full bg-red-100 sm:mx-0 sm:h-10 sm:w-10">
                    <svg className="h-6 w-6 text-red-600" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                    </svg>
                  </div>
                  <div className="mt-3 text-center sm:mt-0 sm:ml-4 sm:text-left">
                    <h3 className="text-lg leading-6 font-medium text-gray-900" id="modal-title">
                      Permanently Delete School Tenant
                    </h3>
                    <div className="mt-2">
                      <p className="text-sm text-gray-500">
                        Are you sure you want to permanently delete this school? All associated data, users, and timetables will be permanently removed. This action cannot be undone.
                      </p>
                      <p className="text-sm text-gray-900 font-medium mt-2">
                        An OTP has been sent to your email and WhatsApp. (Check backend logs for now)
                      </p>
                      <div className="mt-4">
                        <label htmlFor="otp" className="sr-only">OTP</label>
                        <input
                          type="text"
                          name="otp"
                          id="otp"
                          className="shadow-sm focus:ring-blue-500 focus:border-blue-500 block w-full sm:text-sm border-gray-300 rounded-md text-gray-900 py-2 px-3 border"
                          placeholder="Enter 6-digit OTP"
                          value={otp}
                          onChange={(e) => setOtp(e.target.value)}
                        />
                      </div>
                      {deleteError && (
                        <p className="mt-2 text-sm text-red-600">{deleteError}</p>
                      )}
                    </div>
                  </div>
                </div>
              </div>
              <div className="bg-gray-50 px-4 py-3 sm:px-6 sm:flex sm:flex-row-reverse">
                <Button
                  onClick={confirmDelete}
                  loading={deleteLoading}
                  className="w-full inline-flex justify-center rounded-md border border-transparent shadow-sm px-4 py-2 bg-red-600 text-base font-medium text-white hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500 sm:ml-3 sm:w-auto sm:text-sm"
                >
                  Confirm Delete
                </Button>
                <button
                  type="button"
                  className="mt-3 w-full inline-flex justify-center rounded-md border border-gray-300 shadow-sm px-4 py-2 bg-white text-base font-medium text-gray-700 hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 sm:mt-0 sm:ml-3 sm:w-auto sm:text-sm"
                  onClick={() => setDeleteModalOpen(false)}
                >
                  Cancel
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
