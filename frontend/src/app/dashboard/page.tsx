"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";

interface User {
  username: string;
  display_name?: string;
  email_verified: boolean;
  phone_verified: boolean;
  school_status: string;
  roles: string[];
}

export default function DashboardOverview() {
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);
  const [verifyModalOpen, setVerifyModalOpen] = useState(false);
  const [emailOtp, setEmailOtp] = useState("");
  const [phoneOtp, setPhoneOtp] = useState("");
  const [otpLoading, setOtpLoading] = useState(false);
  const [otpError, setOtpError] = useState("");
  
  const [setupMode, setSetupMode] = useState(false);
  const [setupLoading, setSetupLoading] = useState(false);
  const [setupError, setSetupError] = useState("");
  
  const [onboardingStep, setOnboardingStep] = useState("");
  
  const [adminModalOpen, setAdminModalOpen] = useState(false);
  const [adminLoading, setAdminLoading] = useState(false);
  
  const [studentModalOpen, setStudentModalOpen] = useState(false);
  const [studentCsvFile, setStudentCsvFile] = useState<File | null>(null);
  const [studentLoading, setStudentLoading] = useState(false);
  const [studentFailedCsv, setStudentFailedCsv] = useState<string | null>(null);

  
  useEffect(() => {
    if (typeof window !== "undefined") {
      const step = localStorage.getItem("eravaya_onboarding_step");
      if (step) setOnboardingStep(step);
    }
  }, []);

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


  const handleRequestOtp = async () => {
    try {
      setOtpLoading(true);
      const res = await fetch("/api/v1/auth/verify/request-otp", { method: "POST" });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Failed to send OTP";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      setVerifyModalOpen(true);
      setOtpError("");
    } catch (err: any) {
      alert(err.message);
    } finally {
      setOtpLoading(false);
    }
  };


  const handleAddAdmin = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setAdminLoading(true);
    const formData = new FormData(e.currentTarget);
    const payload = Object.fromEntries(formData.entries());
    try {
      const res = await fetch("/api/v1/sis/admins", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      if (!res.ok) throw new Error(await res.text());
      setAdminModalOpen(false);
      localStorage.setItem('eravaya_onboarding_step', 'students');
      setOnboardingStep('students');
    } catch(err) {
      alert(err);
    } finally {
      setAdminLoading(false);
    }
  };

  const handleUploadStudents = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!studentCsvFile) return alert("Please select a file");
    setStudentLoading(true);
    setStudentFailedCsv(null);
    const formData = new FormData();
    formData.append("file", studentCsvFile);
    
    try {
      const res = await fetch("/api/v1/sis/students/bulk-upload", {
        method: "POST",
        body: formData
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Upload failed");
      
      alert(`Successfully added ${data.success_count} students! Failed rows: ${data.failed_count}`);
      if (data.failed_csv) {
        setStudentFailedCsv(data.failed_csv);
      } else {
        setStudentModalOpen(false);
        localStorage.setItem('eravaya_onboarding_step', 'batches');
        setOnboardingStep('batches');
      }
    } catch(err) {
      alert(err);
    } finally {
      setStudentLoading(false);
    }
  };
  
  const downloadFailedCsv = () => {
    if (!studentFailedCsv) return;
    const link = document.createElement("a");
    link.href = `data:text/csv;base64,${studentFailedCsv}`;
    link.download = "failed_student_records.csv";
    link.click();
  };

  const handleSubmitOtp = async () => {
    try {
      setOtpLoading(true);
      setOtpError("");
      const res = await fetch("/api/v1/auth/verify/submit-otp", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email_otp: emailOtp, phone_otp: phoneOtp }),
      });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Invalid OTP";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      setVerifyModalOpen(false);
      setUser(prev => prev ? { ...prev, email_verified: true, phone_verified: true } : null);
      if (user?.school_status === "PENDING_SETUP") {
        setSetupMode(true);
      }
    } catch (err: any) {
      setOtpError(err.message);
    } finally {
      setOtpLoading(false);
    }
  };

  const handleSetupSchool = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setSetupLoading(true);
    setSetupError("");
    const formData = new FormData(e.currentTarget);
    const payload = Object.fromEntries(formData.entries());
    
    try {
      const res = await fetch("/api/v1/auth/schools/setup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Setup failed";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      setUser(prev => prev ? { ...prev, school_status: "ACTIVE" } : null);
      setSetupMode(false);
      if (typeof window !== "undefined") {
        localStorage.setItem("eravaya_onboarding_step", "admins");
        setOnboardingStep("admins");
      }
    } catch (err: any) {
      setSetupError(err.message);
    } finally {
      setSetupLoading(false);
    }
  };

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
              loading={otpLoading}
              onClick={() => {
                if (!user?.email_verified || !user?.phone_verified) {
                  handleRequestOtp();
                } else {
                  setSetupMode(true);
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
        <div className="space-y-6">
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
          
          {user?.roles?.includes("SCHOOL_ADMIN") && onboardingStep && (
            <div className="bg-white rounded-3xl border border-gray-200 p-8 mt-8 shadow-sm">
              <div className="mb-6">
                <h2 className="text-2xl font-bold text-gray-900">School Enrichment Progress</h2>
                <p className="text-gray-500 mt-1">Follow these steps to make your school fully functional (Phase 2 Roadmap)</p>
              </div>
              
              <div className="space-y-4">
                <div className="flex items-center gap-4 p-4 rounded-xl bg-green-50 border border-green-100">
                  <div className="w-8 h-8 rounded-full bg-green-500 text-white flex items-center justify-center shrink-0">✓</div>
                  <div className="flex-1">
                    <h4 className="font-semibold text-green-900">1. Verify Contact Details</h4>
                    <p className="text-sm text-green-700">Email and Phone have been verified.</p>
                  </div>
                </div>
                
                <div className="flex items-center gap-4 p-4 rounded-xl bg-green-50 border border-green-100">
                  <div className="w-8 h-8 rounded-full bg-green-500 text-white flex items-center justify-center shrink-0">✓</div>
                  <div className="flex-1">
                    <h4 className="font-semibold text-green-900">2. School Profile & Location</h4>
                    <p className="text-sm text-green-700">Database tenant initialized and active.</p>
                  </div>
                </div>

                <div className={`flex items-center gap-4 p-4 rounded-xl border ${onboardingStep === 'admins' ? 'bg-blue-50 border-blue-200 shadow-sm' : 'bg-gray-50 border-gray-200 opacity-50'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 ${onboardingStep === 'admins' ? 'bg-blue-600 text-white' : 'bg-gray-300 text-gray-600'}`}>3</div>
                  <div className="flex-1">
                    <h4 className={`font-semibold ${onboardingStep === 'admins' ? 'text-blue-900' : 'text-gray-700'}`}>3. Add Administrators & Staff</h4>
                    <p className="text-sm text-gray-500">Invite principals, HR, and teachers into the system.</p>
                  </div>
                  {onboardingStep === 'admins' && (
                    <div className="flex gap-2">
                      <Button variant="secondary" onClick={() => {
                        localStorage.setItem('eravaya_onboarding_step', 'students');
                        setOnboardingStep('students');
                      }}>Skip</Button>
                      <Button onClick={() => setAdminModalOpen(true)}>Add Admin</Button>
                    </div>
                  )}
                </div>

                <div className={`flex items-center gap-4 p-4 rounded-xl border ${onboardingStep === 'students' ? 'bg-blue-50 border-blue-200 shadow-sm' : 'bg-gray-50 border-gray-200 opacity-50'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 ${onboardingStep === 'students' ? 'bg-blue-600 text-white' : 'bg-gray-300 text-gray-600'}`}>4</div>
                  <div className="flex-1">
                    <h4 className={`font-semibold ${onboardingStep === 'students' ? 'text-blue-900' : 'text-gray-700'}`}>4. Enroll Students</h4>
                    <p className="text-sm text-gray-500">Import student directories and parent contacts.</p>
                  </div>
                  {onboardingStep === 'students' && (
                    <Button onClick={() => setStudentModalOpen(true)}>Upload CSV</Button>
                  )}
                </div>

                <div className={`flex items-center gap-4 p-4 rounded-xl border ${onboardingStep === 'batches' ? 'bg-blue-50 border-blue-200 shadow-sm' : 'bg-gray-50 border-gray-200 opacity-50'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 ${onboardingStep === 'batches' ? 'bg-blue-600 text-white' : 'bg-gray-300 text-gray-600'}`}>5</div>
                  <div className="flex-1">
                    <h4 className={`font-semibold ${onboardingStep === 'batches' ? 'text-blue-900' : 'text-gray-700'}`}>5. Create Classes & Batches</h4>
                    <p className="text-sm text-gray-500">Define academic structure and section divisions.</p>
                  </div>
                  {onboardingStep === 'batches' && (
                    <Button onClick={() => {
                      localStorage.setItem('eravaya_onboarding_step', 'timetable');
                      setOnboardingStep('timetable');
                    }}>Mark Complete</Button>
                  )}
                </div>
                
                <div className={`flex items-center gap-4 p-4 rounded-xl border ${onboardingStep === 'timetable' ? 'bg-blue-50 border-blue-200 shadow-sm' : 'bg-gray-50 border-gray-200 opacity-50'}`}>
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 ${onboardingStep === 'timetable' ? 'bg-blue-600 text-white' : 'bg-gray-300 text-gray-600'}`}>6</div>
                  <div className="flex-1">
                    <h4 className={`font-semibold ${onboardingStep === 'timetable' ? 'text-blue-900' : 'text-gray-700'}`}>6. Generate Timetable</h4>
                    <p className="text-sm text-gray-500">Configure scheduling rules and dispatch final timetable.</p>
                  </div>
                  {onboardingStep === 'timetable' && (
                    <Button onClick={() => {
                      localStorage.removeItem('eravaya_onboarding_step');
                      setOnboardingStep('');
                      alert('Phase 1 Onboarding Complete!');
                    }}>Finish Setup</Button>
                  )}
                </div>
                
              </div>
            </div>
          )}
        </div>
      )}

      {/* Verify OTP Modal */}
      {verifyModalOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-gray-900/60 backdrop-blur-sm">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-md overflow-hidden animate-in zoom-in-95 duration-200 p-6">
            <h3 className="text-xl font-bold text-gray-900 mb-2">Verify your Account</h3>
            <p className="text-gray-600 text-sm mb-4">
              A 6-digit verification code has been sent to your email and WhatsApp (terminal).
            </p>
            <div className="mb-4 space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">Email OTP</label>
                <input
                  type="text"
                  className="w-full border-gray-300 rounded-lg shadow-sm focus:border-blue-500 focus:ring-blue-500 p-3 border text-gray-900 font-mono text-center tracking-widest text-xl"
                  placeholder="------"
                  maxLength={6}
                  value={emailOtp}
                  onChange={(e) => setEmailOtp(e.target.value.toUpperCase())}
                />
              </div>
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">WhatsApp OTP</label>
                <input
                  type="text"
                  className="w-full border-gray-300 rounded-lg shadow-sm focus:border-green-500 focus:ring-green-500 p-3 border text-gray-900 font-mono text-center tracking-widest text-xl"
                  placeholder="------"
                  maxLength={6}
                  value={phoneOtp}
                  onChange={(e) => setPhoneOtp(e.target.value.toUpperCase())}
                />
              </div>
            </div>
            {otpError && <p className="text-red-600 text-sm font-medium text-center mb-4">{otpError}</p>}
            <div className="flex gap-3">
              <Button variant="secondary" fullWidth onClick={() => setVerifyModalOpen(false)}>Cancel</Button>
              <Button variant="primary" fullWidth loading={otpLoading} onClick={handleSubmitOtp}>Verify</Button>
            </div>
          </div>
        </div>
      )}

      {/* Setup Form Modal / View */}
      {setupMode && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-gray-900/60 backdrop-blur-sm overflow-y-auto">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-3xl overflow-hidden animate-in zoom-in-95 duration-200 p-8 my-8">
            <div className="flex justify-between items-center mb-6">
              <div>
                <h2 className="text-2xl font-bold text-gray-900 mb-2">Complete School Profile</h2>
                <p className="text-sm text-gray-500">Add the foundational details of your institution to unlock all modules.</p>
              </div>
              <button onClick={() => setSetupMode(false)} className="text-gray-400 hover:text-gray-600">✕</button>
            </div>
            
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
              
              {setupError && <p className="text-red-600 text-sm font-medium mt-4">{setupError}</p>}
              
              <div className="pt-6 border-t flex justify-end gap-4">
                <Button variant="secondary" onClick={() => setSetupMode(false)} type="button">Cancel</Button>
                <Button variant="primary" type="submit" loading={setupLoading}>Save & Activate School</Button>
              </div>
            </form>
          </div>
        </div>
      )}


      {/* Admin Modal */}
      {adminModalOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-gray-900/60 backdrop-blur-sm">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-md overflow-hidden p-6">
            <h3 className="text-xl font-bold mb-4">Add Administrator</h3>
            <form onSubmit={handleAddAdmin} className="space-y-4">
              <Input required name="first_name" label="First Name" />
              <Input required name="last_name" label="Last Name" />
              <Input required name="email" type="email" label="Email" />
              <Input required name="phone" type="tel" label="Phone" />
              <div className="flex justify-end gap-3 pt-4">
                <Button variant="secondary" type="button" onClick={() => setAdminModalOpen(false)}>Cancel</Button>
                <Button variant="primary" type="submit" loading={adminLoading}>Add Admin</Button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Student CSV Modal */}
      {studentModalOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-gray-900/60 backdrop-blur-sm">
          <div className="bg-white rounded-2xl shadow-xl w-full max-w-lg overflow-hidden p-6">
            <h3 className="text-xl font-bold mb-2">Bulk Enroll Students</h3>
            <p className="text-sm text-gray-500 mb-6">Upload a CSV containing student and guardian data.</p>
            
            <div className="mb-6 p-4 bg-blue-50 text-blue-800 rounded-lg text-sm border border-blue-100">
              <p className="font-semibold mb-2">Required CSV Headers:</p>
              <ul className="list-disc pl-5 space-y-1">
                <li>First Name</li>
                <li>Last Name</li>
                <li>Admission Number</li>
                <li>Date of Birth (YYYY-MM-DD)</li>
                <li>Guardian First Name</li>
                <li>Guardian Last Name</li>
                <li>Guardian Email</li>
                <li>Guardian Phone</li>
                <li>Student Phone <span className="text-gray-500 font-normal">(Optional)</span></li>
              </ul>
            </div>
            
            <form onSubmit={handleUploadStudents} className="space-y-4">
              <input type="file" accept=".csv" onChange={(e) => setStudentCsvFile(e.target.files?.[0] || null)} className="w-full border p-2 rounded" />
              
              {studentFailedCsv && (
                <div className="p-4 bg-red-50 border border-red-200 rounded-lg">
                  <p className="text-red-700 text-sm font-semibold mb-2">Some rows failed validation!</p>
                  <Button variant="secondary" type="button" onClick={downloadFailedCsv} className="w-full border-red-300 text-red-700 hover:bg-red-100">
                    Download Incomplete Data (CSV)
                  </Button>
                </div>
              )}
              
              <div className="flex justify-end gap-3 pt-4">
                <Button variant="secondary" type="button" onClick={() => setStudentModalOpen(false)}>Cancel</Button>
                <Button variant="primary" type="submit" loading={studentLoading}>Upload & Process</Button>
              </div>
            </form>
          </div>
        </div>
      )}

    </div>
  );
}
