import Link from "next/link";

export default function LandingPage() {
  return (
    <div className="min-h-screen bg-white text-gray-900 selection:bg-[#3366FF]/20 flex flex-col font-sans">
      {/* Navbar */}
      <header className="sticky top-0 z-50 bg-white/80 backdrop-blur-xl border-b border-gray-100">
        <div className="max-w-6xl mx-auto px-6 h-16 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 bg-[#0C152E] rounded-md flex items-center justify-center">
              <span className="text-white font-bold text-sm">E</span>
            </div>
            <span className="font-semibold tracking-tight text-lg">Eravaya</span>
          </div>
          <nav className="flex items-center gap-6">
            <Link href="/login" className="text-sm font-medium text-gray-600 hover:text-gray-900 transition-colors">
              Sign in
            </Link>
            <Link href="/login" className="text-sm font-medium bg-[#0C152E] text-white px-4 py-2 rounded-full hover:bg-gray-800 transition-all shadow-sm">
              Get Started
            </Link>
          </nav>
        </div>
      </header>

      {/* Hero Section */}
      <main className="flex-1 flex flex-col items-center justify-center text-center px-6 py-20 max-w-4xl mx-auto mt-10">
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-50 border border-blue-100 text-blue-700 text-xs font-medium mb-8">
          <span className="relative flex h-2 w-2">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-blue-400 opacity-75"></span>
            <span className="relative inline-flex rounded-full h-2 w-2 bg-blue-500"></span>
          </span>
          Eravaya ERP 2.0 is now live
        </div>
        
        <h1 className="text-5xl md:text-7xl font-bold tracking-tight text-gray-900 leading-[1.1] mb-6">
          The operating system <br className="hidden md:block" />
          <span className="text-transparent bg-clip-text bg-gradient-to-r from-[#3366FF] to-[#00C2FF]">for modern schools.</span>
        </h1>
        
        <p className="text-lg md:text-xl text-gray-500 max-w-2xl mb-10 leading-relaxed">
          Manage admissions, academics, HR, and finances in one unified platform. 
          Built for speed, designed for clarity.
        </p>
        
        <div className="flex flex-col sm:flex-row items-center gap-4">
          <Link href="/login" className="w-full sm:w-auto text-base font-medium bg-[#3366FF] hover:bg-[#2B57D9] text-white px-8 py-3.5 rounded-full transition-all shadow-lg shadow-blue-500/20">
            Access Dashboard
          </Link>
          <a href="#features" className="w-full sm:w-auto text-base font-medium bg-white hover:bg-gray-50 text-gray-900 border border-gray-200 px-8 py-3.5 rounded-full transition-all shadow-sm">
            Explore Features
          </a>
        </div>
      </main>
      
      <footer className="mt-auto py-8 border-t border-gray-100 text-center text-sm text-gray-500">
        <p>&copy; {new Date().getFullYear()} Eravaya Technologies. All rights reserved.</p>
      </footer>
    </div>
  );
}
