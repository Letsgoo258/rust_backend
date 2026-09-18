"use client";

export default function DashboardOverview() {
  return (
    <div className="p-8 md:p-12 max-w-6xl mx-auto animate-in fade-in slide-in-from-bottom-4 duration-500">
      <header className="mb-10">
        <h1 className="text-3xl font-semibold text-gray-900 tracking-tight mb-2">Good morning</h1>
        <p className="text-gray-500 text-lg">Here is what's happening at your school today.</p>
      </header>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm">
          <h3 className="text-sm font-medium text-gray-500 mb-1">Total Students</h3>
          <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
        </div>
        <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm">
          <h3 className="text-sm font-medium text-gray-500 mb-1">Active Classes</h3>
          <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
        </div>
        <div className="bg-white/60 backdrop-blur-xl border border-white p-6 rounded-3xl shadow-sm">
          <h3 className="text-sm font-medium text-gray-500 mb-1">Pending Enrollments</h3>
          <p className="text-4xl font-semibold text-gray-900 tracking-tight">0</p>
        </div>
      </div>
    </div>
  );
}
