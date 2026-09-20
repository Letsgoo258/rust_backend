"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

let sessionId = "";
if (typeof window !== "undefined") {
  sessionId = sessionStorage.getItem("eravaya_session") || crypto.randomUUID();
  sessionStorage.setItem("eravaya_session", sessionId);
}

function sendTelemetry(eventType: string, data: any) {
  if (typeof window === "undefined") return;
  
  // Use sendBeacon for reliable delivery even during page unload
  const payload = {
    session_id: sessionId,
    event_type: eventType,
    url: window.location.pathname,
    ...data,
  };
  
  const blob = new Blob([JSON.stringify(payload)], { type: "application/json" });
  navigator.sendBeacon("/api/v1/telemetry/event", blob);
}

export function AnalyticsProvider({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const [sessionStartTime] = useState(Date.now());

  // Track page views
  useEffect(() => {
    sendTelemetry("PAGE_VIEW", {});
  }, [pathname]);

  // Track global errors
  useEffect(() => {
    const handleError = (e: ErrorEvent) => {
      sendTelemetry("FRONTEND_ERROR", {
        error_message: e.message,
        error_stack: e.error?.stack,
        component: "Global Window",
      });
    };
    
    const handleRejection = (e: PromiseRejectionEvent) => {
      sendTelemetry("FRONTEND_ERROR", {
        error_message: e.reason?.message || "Unhandled Promise Rejection",
        error_stack: e.reason?.stack,
        component: "Global Window",
      });
    };

    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleRejection);
    
    return () => {
      window.removeEventListener("error", handleError);
      window.removeEventListener("unhandledrejection", handleRejection);
    };
  }, []);

  // Track clicks globally on buttons and links
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const buttonOrLink = target.closest('button') || target.closest('a');
      
      // Only track buttons and links
      if (buttonOrLink) {
        let name = buttonOrLink.innerText || buttonOrLink.getAttribute('aria-label') || buttonOrLink.tagName;
        if (name.length > 50) name = name.substring(0, 50) + "...";
        
        // Safe to enable now since backend uses a file buffer instead of live DB writes!
        sendTelemetry("USERCLICK", { component: name });
      }
    };

    window.addEventListener("click", handleClick, { capture: true });
    return () => window.removeEventListener("click", handleClick, { capture: true });
  }, []);

  // Track session duration on unload
  useEffect(() => {
    const handleUnload = () => {
      const durationMs = Date.now() - sessionStartTime;
      sendTelemetry("SESSION_DURATION", {
        duration_ms: durationMs,
      });
    };
    window.addEventListener("beforeunload", handleUnload);
    return () => window.removeEventListener("beforeunload", handleUnload);
  }, [sessionStartTime]);

  return <>{children}</>;
}
