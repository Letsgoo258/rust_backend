"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { Button } from "@/components/ui/Button";

export default function VerifyAccountPage() {
  const router = useRouter();
  const [loading, setLoading] = useState(false);
  const [otpSent, setOtpSent] = useState(false);
  const [emailOtp, setEmailOtp] = useState("");
  const [phoneOtp, setPhoneOtp] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [user, setUser] = useState<any>(null);

  useEffect(() => {
    // Check if user is logged in
    fetch("/api/v1/auth/me")
      .then(res => {
        if (!res.ok) throw new Error();
        return res.json();
      })
      .then(data => {
        const u = data.user || data;
        if (!u || !u.id) {
          router.push("/login");
          return;
        }
        if (u.email_verified && u.phone_verified) {
          if (u.is_tenant_admin && !u.school_operable) {
            router.push("/dataenrich");
          } else {
            router.push("/dashboard");
          }
          return;
        }
        setUser(u);
      })
      .catch(() => router.push("/login"));
  }, [router]);

  const handleSendOtp = async () => {
    try {
      setLoading(true);
      setErrorMsg("");
      const res = await fetch("/api/v1/auth/verify/request-otp", { method: "POST" });
      if (!res.ok) {
        const text = await res.text();
        let errMsg = text || "Failed to send OTP";
        try { errMsg = JSON.parse(text).error || text; } catch (e) {}
        throw new Error(errMsg);
      }
      setOtpSent(true);
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  const handleVerify = async () => {
    if (!emailOtp || !phoneOtp) {
      setErrorMsg("Please enter both OTPs");
      return;
    }
    
    try {
      setLoading(true);
      setErrorMsg("");
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
      
      // Success, route to dataenrich or dashboard
      if (user?.is_tenant_admin && !user?.school_operable) {
        router.push("/dataenrich");
      } else {
        router.push("/dashboard");
      }
    } catch (err: any) {
      setErrorMsg(err.message);
    } finally {
      setLoading(false);
    }
  };

  if (!user) return <div className="min-h-screen flex items-center justify-center bg-gray-50">Loading...</div>;

  return (
    <div className="min-h-screen flex w-full bg-gray-50 items-center justify-center p-4">
      <div className="bg-white rounded-2xl shadow-xl w-full max-w-md p-8">
        <div className="text-center mb-8">
          <div className="w-12 h-12 bg-blue-100 text-blue-600 rounded-full flex items-center justify-center mx-auto mb-4">
            <svg className="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <h2 className="text-2xl font-bold text-gray-900 mb-2">Verify Your Account</h2>
          <p className="text-gray-500 text-sm">
            For security reasons, please verify your contact information to activate your account.
          </p>
        </div>

        {errorMsg && (
          <div className="mb-6 p-3 bg-red-50 border border-red-100 text-red-600 text-sm rounded-lg text-center font-medium">
            {errorMsg}
          </div>
        )}

        {!otpSent ? (
          <div className="space-y-6">
            <div className="bg-gray-50 p-4 rounded-lg space-y-3">
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Email:</span>
                <span className="text-sm font-medium text-gray-900">{user.email || "Not Provided"}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-sm text-gray-500">Phone:</span>
                <span className="text-sm font-medium text-gray-900">{user.phone || "Not Provided"}</span>
              </div>
            </div>
            
            <Button onClick={handleSendOtp} loading={loading} fullWidth>
              Send Verification Codes
            </Button>
          </div>
        ) : (
          <div className="space-y-6">
            <p className="text-sm text-gray-600 text-center mb-4">
              We've sent one-time passwords (OTP) to your email and phone number.
            </p>
            
            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 mb-1">Email OTP</label>
                <input
                  type="text"
                  className="w-full border-gray-300 rounded-lg shadow-sm focus:border-blue-500 focus:ring-blue-500 p-3 border text-gray-900 font-mono text-center tracking-widest text-xl uppercase"
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
                  className="w-full border-gray-300 rounded-lg shadow-sm focus:border-green-500 focus:ring-green-500 p-3 border text-gray-900 font-mono text-center tracking-widest text-xl uppercase"
                  placeholder="------"
                  maxLength={6}
                  value={phoneOtp}
                  onChange={(e) => setPhoneOtp(e.target.value.toUpperCase())}
                />
              </div>
            </div>
            
            <Button onClick={handleVerify} loading={loading} fullWidth className="mt-6">
              Verify & Continue
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
