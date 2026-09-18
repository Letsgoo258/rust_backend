"use client";

import { ButtonHTMLAttributes, ReactNode } from "react";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  children: ReactNode;
  loading?: boolean;
  loadingText?: string;
  variant?: "primary" | "secondary" | "outline" | "danger";
  fullWidth?: boolean;
}

export function Button({
  children,
  loading = false,
  loadingText = "Please wait...",
  variant = "primary",
  fullWidth = false,
  className = "",
  disabled,
  ...props
}: ButtonProps) {
  const baseStyles = "relative font-semibold py-3 rounded-lg transition-all text-[15px] shadow-sm flex items-center justify-center gap-2 outline-none";
  
  const variants = {
    primary: "bg-[#3366FF] hover:bg-[#2B57D9] text-white disabled:bg-[#3366FF]/70 focus:ring-2 focus:ring-[#3366FF]/50 focus:ring-offset-1",
    secondary: "bg-gray-100 hover:bg-gray-200 text-gray-900 disabled:bg-gray-100/70",
    outline: "bg-white hover:bg-gray-50 border border-gray-300 text-gray-700 disabled:bg-gray-50",
    danger: "bg-red-600 hover:bg-red-700 text-white disabled:bg-red-600/70",
  };

  const widthStyle = fullWidth ? "w-full" : "w-auto px-6";
  const isDisabled = disabled || loading;

  return (
    <button
      disabled={isDisabled}
      className={`${baseStyles} ${variants[variant]} ${widthStyle} ${className}`}
      {...props}
    >
      {loading ? (
        <>
          <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-current" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
          {loadingText}
        </>
      ) : (
        children
      )}
    </button>
  );
}
