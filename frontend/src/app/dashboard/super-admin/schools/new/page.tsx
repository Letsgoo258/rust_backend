"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

export default function NewSchoolPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [success, setSuccess] = useState(false);

  const handleCreate = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setLoading(true);
    setErrorMsg("");

    const formData = new FormData(e.currentTarget);
    const data = Object.fromEntries(formData.entries());

    try {
      const res = await fetch("/api/v1/auth/schools", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data),
      });

      if (!res.ok) {
        const error = await res.json();
        throw new Error(error.error || "Failed to create school");
      }
      
      setSuccess(true);
      setTimeout(() => {
        router.push("/dashboard/super-admin");
      }, 2000);
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="max-w-2xl mx-auto py-8 px-4">
      <div className="mb-8">
        <h1 className="text-2xl font-semibold tracking-tight text-gray-900">Add New School</h1>
        <p className="text-gray-500 text-sm mt-1">Register a new school tenant in the Eravaya platform.</p>
      </div>

      {success ? (
        <div className="p-6 bg-green-50 border border-green-100 rounded-xl text-green-800 text-center">
          <h3 className="font-semibold text-lg mb-1">School Created Successfully!</h3>
          <p className="text-sm">Redirecting back to dashboard...</p>
        </div>
      ) : (
        <form onSubmit={handleCreate} className="space-y-6 bg-white border border-gray-200 rounded-2xl p-6 shadow-sm">
          {errorMsg && (
            <div className="p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg">
              {errorMsg}
            </div>
          )}

          <div className="space-y-6">
            
            <div>
              <h3 className="text-lg font-medium text-gray-900 border-b pb-2 mb-4">School Details</h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">School Name</label>
                  <input required name="name" type="text" placeholder="Greenwood High" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">School Code (Unique)</label>
                  <input required name="code" type="text" placeholder="GWH01" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm uppercase" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">School Email (Optional)</label>
                  <input name="email" type="email" placeholder="contact@greenwood.edu" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">Phone Number (Optional)</label>
                  <input name="phone" type="tel" placeholder="+1 234 567 8900" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>
              </div>
            </div>

            <div>
              <h3 className="text-lg font-medium text-gray-900 border-b pb-2 mb-4">School Admin Account</h3>
              <p className="text-sm text-gray-500 mb-4">This user will log in to configure the rest of the school's data.</p>
              
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">First Name</label>
                  <input required name="admin_first_name" type="text" placeholder="Jane" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">Last Name</label>
                  <input required name="admin_last_name" type="text" placeholder="Doe" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">Admin Username</label>
                  <input required name="admin_username" type="text" placeholder="admin_gwh" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5">
                  <label className="block text-sm font-medium text-gray-700">Admin Email</label>
                  <input required name="admin_email" type="email" placeholder="jane@greenwood.edu" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>

                <div className="space-y-1.5 md:col-span-2">
                  <label className="block text-sm font-medium text-gray-700">Temporary Password</label>
                  <input required name="admin_password" type="password" placeholder="••••••••" className="w-full bg-white border border-gray-200 focus:border-[#3366FF] focus:ring-1 focus:ring-[#3366FF] outline-none rounded-lg px-3 py-2 text-sm" />
                </div>
              </div>
            </div>

          </div>

          <div className="pt-4 border-t border-gray-100 flex justify-end">
            <Button type="submit" loading={loading} loadingText="Creating...">
              Create School
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
