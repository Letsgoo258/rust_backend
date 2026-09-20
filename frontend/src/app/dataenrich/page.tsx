"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";

export default function DataEnrichPage() {
  const router = useRouter();
  const [user, setUser] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [step, setStep] = useState<number>(1);

  const [setupLoading, setSetupLoading] = useState(false);
  const [setupError, setSetupError] = useState("");

  const [studentCsvFile, setStudentCsvFile] = useState<File | null>(null);
  const [uploadLoading, setUploadLoading] = useState(false);
  const [uploadMessage, setUploadMessage] = useState("");
  const [failedCsv, setFailedCsv] = useState<string | null>(null);

  useEffect(() => {
    fetch("/api/v1/auth/me")
      .then((res) => {
        if (!res.ok) throw new Error("Not logged in");
        return res.json();
      })
      .then((data) => {
        const u = data.user || data;
        if (!u || !u.id) {
          router.push("/login");
          return;
        }
        if (!u.email_verified || !u.phone_verified) {
          router.push("/verifyacc");
          return;
        }
        if (u.is_tenant_admin && u.school_operable) {
          router.push("/dashboard");
          return;
        }
        setUser(u);
        setStep(u.school_onboarding_step || 1);
        setLoading(false);
      })
      .catch(() => {
        router.push("/login");
      });
  }, [router]);

  const handleNextStep = async (nextStep: number) => {
    try {
      setLoading(true);
      const res = await fetch("/api/v1/auth/schools/onboarding-step", {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ step: nextStep }),
      });
      if (res.ok) {
        setStep(nextStep);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleSaveSchoolData = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setSetupLoading(true);
    setSetupError("");

    const formData = new FormData(e.currentTarget);
    const payload = Object.fromEntries(formData.entries());

    try {
      const res = await fetch("/api/v1/auth/schools/setup", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Failed to save school details";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }

      setStep(3);
    } catch (err: any) {
      setSetupError(err.message);
    } finally {
      setSetupLoading(false);
    }
  };

  const handleUploadStudents = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!studentCsvFile) return;

    setUploadLoading(true);
    setUploadMessage("");
    setFailedCsv(null);

    const formData = new FormData();
    formData.append("file", studentCsvFile);

    try {
      const res = await fetch("/api/v1/users/students/bulk-upload", {
        method: "POST",
        body: formData,
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error || "Upload failed");

      setUploadMessage(data.message || "Students successfully uploaded!");
      if (data.errors_base64) {
        setFailedCsv(data.errors_base64);
      }
    } catch (err: any) {
      setUploadMessage(`Error: ${err.message}`);
    } finally {
      setUploadLoading(false);
    }
  };

  const handleCompleteOnboarding = async () => {
    try {
      setLoading(true);
      const res = await fetch("/api/v1/auth/schools/complete-onboarding", {
        method: "POST",
      });
      if (res.ok) {
        router.push("/dashboard");
      } else {
        alert("Failed to unlock dashboard. Please complete all steps.");
      }
    } finally {
      setLoading(false);
    }
  };

  const downloadCsvTemplate = () => {
    const headers = "First Name,Last Name,Admission Number,Date of Birth,Guardian First Name,Guardian Last Name,Guardian Email,Guardian Phone,Student Phone\nRahul,Sharma,ADM-2026-001,2012-05-15,Vikram,Sharma,vikram@example.com,9876543210,9876543211";
    const blob = new Blob([headers], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "student_onboarding_template.csv";
    a.click();
    URL.revokeObjectURL(url);
  };

  if (loading || !user) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-gray-50">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600"></div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col justify-between p-6 md:p-12 font-sans">
      <div className="max-w-4xl mx-auto w-full">
        
        {/* TOP HEADER */}
        <div className="mb-10 text-center md:text-left flex flex-col md:flex-row justify-between items-center border-b pb-6">
          <div>
            <div className="inline-flex items-center gap-2 px-3 py-1 bg-blue-100 text-blue-700 rounded-full text-xs font-semibold mb-2">
              School Setup Wizard
            </div>
            <h1 className="text-3xl font-bold text-gray-900 tracking-tight">
              Data Enrichment &amp; System Activation
            </h1>
            <p className="text-gray-500 text-sm mt-1">
              Complete these 3 foundational steps to make your school operable.
            </p>
          </div>
          
          {/* STEP INDICATOR BADGES */}
          <div className="flex items-center gap-2 mt-4 md:mt-0">
            <span className={`w-8 h-8 rounded-full flex items-center justify-center font-bold text-xs ${step >= 1 ? "bg-blue-600 text-white" : "bg-gray-200 text-gray-500"}`}>1</span>
            <div className={`w-6 h-0.5 ${step >= 2 ? "bg-blue-600" : "bg-gray-200"}`}></div>
            <span className={`w-8 h-8 rounded-full flex items-center justify-center font-bold text-xs ${step >= 2 ? "bg-blue-600 text-white" : "bg-gray-200 text-gray-500"}`}>2</span>
            <div className={`w-6 h-0.5 ${step >= 3 ? "bg-blue-600" : "bg-gray-200"}`}></div>
            <span className={`w-8 h-8 rounded-full flex items-center justify-center font-bold text-xs ${step >= 3 ? "bg-blue-600 text-white" : "bg-gray-200 text-gray-500"}`}>3</span>
          </div>
        </div>

        {/* STEP 1: WELCOME INFO */}
        {step === 1 && (
          <div className="bg-white rounded-3xl p-8 md:p-12 shadow-sm border border-gray-200 animate-in fade-in duration-300">
            <h2 className="text-2xl font-bold text-gray-900 mb-4">Step 1: System Welcome &amp; Onboarding Overview</h2>
            <p className="text-gray-600 text-base leading-relaxed mb-6">
              Welcome to <strong>Eravaya School ERP</strong>! As a School Administrator, you are initializing your school's secure tenant.
              Before accessing your full operations dashboard, you must complete your school profile and enrich student records.
            </p>
            
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-8">
              <div className="p-4 bg-blue-50 border border-blue-100 rounded-2xl">
                <h4 className="font-semibold text-blue-900 mb-1">1. Information</h4>
                <p className="text-xs text-blue-700">Understand tenant setup procedures.</p>
              </div>
              <div className="p-4 bg-gray-50 border border-gray-200 rounded-2xl">
                <h4 className="font-semibold text-gray-800 mb-1">2. School Data</h4>
                <p className="text-xs text-gray-500">Provide official contact &amp; location details.</p>
              </div>
              <div className="p-4 bg-gray-50 border border-gray-200 rounded-2xl">
                <h4 className="font-semibold text-gray-800 mb-1">3. Add Students</h4>
                <p className="text-xs text-gray-500">Bulk import student directories &amp; parents.</p>
              </div>
            </div>

            <div className="flex justify-end pt-4 border-t">
              <Button onClick={() => handleNextStep(2)} className="bg-blue-600 hover:bg-blue-700 text-white px-8 py-3 rounded-xl">
                Continue to Step 2: School Data &rarr;
              </Button>
            </div>
          </div>
        )}

        {/* STEP 2: SCHOOL DATA */}
        {step === 2 && (
          <div className="bg-white rounded-3xl p-8 md:p-12 shadow-sm border border-gray-200 animate-in fade-in duration-300">
            <h2 className="text-2xl font-bold text-gray-900 mb-2">Step 2: Fill Official School Profile Data</h2>
            <p className="text-gray-500 text-sm mb-6">Provide your school's location and regional configuration.</p>

            <form onSubmit={handleSaveSchoolData} className="space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                <Input required name="short_name" type="text" label="Short Name (e.g. DPS-DELHI)" />
                <Input name="website" type="url" label="Website URL (Optional)" placeholder="https://" />
              </div>

              <h3 className="text-md font-semibold text-gray-900 border-b pb-2 pt-2">Address &amp; Regional Location</h3>
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

              {setupError && (
                <div className="p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg">
                  {setupError}
                </div>
              )}

              <div className="flex justify-between items-center pt-6 border-t">
                <Button variant="secondary" type="button" onClick={() => setStep(1)}>
                  &larr; Back
                </Button>
                <Button type="submit" loading={setupLoading} className="bg-blue-600 hover:bg-blue-700 text-white px-8 py-3 rounded-xl">
                  Save &amp; Continue to Step 3 &rarr;
                </Button>
              </div>
            </form>
          </div>
        )}

        {/* STEP 3: ADDING STUDENTS & STAFF */}
        {step === 3 && (
          <div className="bg-white rounded-3xl p-8 md:p-12 shadow-sm border border-gray-200 animate-in fade-in duration-300">
            <h2 className="text-2xl font-bold text-gray-900 mb-2">Step 3: Add Students &amp; Staff Directory</h2>
            <p className="text-gray-500 text-sm mb-6">Upload student directory CSV or prepare data structure.</p>

            <div className="mb-6 p-6 bg-blue-50 text-blue-900 rounded-2xl border border-blue-100">
              <div className="flex flex-col md:flex-row justify-between items-start md:items-center gap-4 mb-4">
                <div>
                  <h4 className="font-bold text-base">Required CSV Headers &amp; Format</h4>
                  <p className="text-xs text-blue-700">Download the pre-formatted CSV template file to ensure zero errors during import.</p>
                </div>
                <button
                  type="button"
                  onClick={downloadCsvTemplate}
                  className="px-4 py-2 bg-blue-600 text-white text-xs font-semibold rounded-xl hover:bg-blue-700 transition flex items-center gap-2 shadow-sm shrink-0"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path></svg>
                  Download CSV Template
                </button>
              </div>

              <div className="flex flex-wrap gap-2">
                {["First Name*", "Last Name*", "Admission Number*", "Date of Birth*", "Guardian First Name*", "Guardian Last Name*", "Guardian Email*", "Guardian Phone*"].map((header) => (
                  <span key={header} className="px-2.5 py-1 bg-white border border-blue-200 rounded-lg text-xs font-mono font-medium text-blue-800">
                    {header}
                  </span>
                ))}
              </div>
            </div>

            <form onSubmit={handleUploadStudents} className="space-y-6 mb-8">
              <div className="border-2 border-dashed border-gray-300 rounded-2xl p-6 text-center hover:border-blue-400 transition bg-gray-50">
                <input
                  type="file"
                  accept=".csv"
                  className="block w-full text-sm text-gray-500 file:mr-4 file:py-2 file:px-4 file:rounded-xl file:border-0 file:text-xs file:font-semibold file:bg-blue-50 file:text-blue-700 hover:file:bg-blue-100 cursor-pointer"
                  onChange={(e) => setStudentCsvFile(e.target.files?.[0] || null)}
                />
              </div>

              <div className="flex justify-between items-center">
                <Button type="submit" loading={uploadLoading} disabled={!studentCsvFile} className="bg-gray-900 text-white px-6 py-2.5 rounded-xl">
                  Upload Student CSV
                </Button>
                {uploadMessage && (
                  <span className="text-xs font-semibold text-blue-600">{uploadMessage}</span>
                )}
              </div>
            </form>

            <div className="pt-8 border-t flex flex-col md:flex-row justify-between items-center gap-4">
              <Button variant="secondary" onClick={() => setStep(2)}>
                &larr; Back to Step 2
              </Button>
              <Button
                onClick={handleCompleteOnboarding}
                className="bg-green-600 hover:bg-green-700 text-white font-bold px-8 py-3.5 rounded-xl shadow-lg shadow-green-600/20 w-full md:w-auto"
              >
                Complete Onboarding &amp; Unlock Dashboard &#10003;
              </Button>
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
