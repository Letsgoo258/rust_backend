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
                  <th className="px-6 py-3 font-medium">Created</th>
                  <th className="px-6 py-3 font-medium text-right">Actions</th>
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
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-gray-900/60 backdrop-blur-sm">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-md overflow-hidden animate-in zoom-in-95 duration-200">
            <div className="p-6">
              <div className="flex items-center gap-4 mb-4">
                <div className="w-12 h-12 rounded-full bg-red-100 flex items-center justify-center shrink-0">
                  <svg className="w-6 h-6 text-red-600" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" /></svg>
                </div>
                <h3 className="text-xl font-bold text-gray-900">Permanent Deletion</h3>
              </div>
              
              <p className="text-gray-600 text-sm mb-4">
                Are you sure you want to permanently delete this school? All associated data, users, and timetables will be <strong className="text-gray-900">permanently removed</strong>. This action cannot be undone.
              </p>
              
              <div className="bg-yellow-50 text-yellow-800 text-xs p-3 rounded-lg border border-yellow-200 mb-4">
                An OTP has been sent to your email address.
              </div>
              
              <div className="mb-4">
                <label className="block text-sm font-medium text-gray-700 mb-1">Enter 6-digit OTP Code</label>
                <input
                  type="text"
                  className="w-full border-gray-300 rounded-lg shadow-sm focus:border-red-500 focus:ring-red-500 p-2.5 border text-gray-900 font-mono text-center tracking-widest text-lg"
                  placeholder="------"
                  maxLength={6}
                  value={otp}
                  onChange={(e) => setOtp(e.target.value.toUpperCase())}
                />
              </div>

              {deleteError && (
                <p className="text-red-600 text-sm font-medium text-center mb-4">{deleteError}</p>
              )}

              <div className="flex gap-3 mt-6">
                <Button variant="secondary" fullWidth onClick={() => setDeleteModalOpen(false)}>Cancel</Button>
                <Button variant="danger" fullWidth loading={deleteLoading} onClick={confirmDelete}>Confirm Delete</Button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}