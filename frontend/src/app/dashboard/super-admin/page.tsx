import Link from "next/link";

export default function SuperAdminDashboard() {
  return (
    <div className="max-w-5xl mx-auto py-8 px-4">
      <div className="mb-8">
        <h1 className="text-2xl font-semibold tracking-tight text-gray-900">Platform Administration</h1>
        <p className="text-gray-500 text-sm mt-1">Manage global platform settings and school tenants.</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="bg-white border border-gray-200 rounded-2xl p-6 shadow-sm hover:shadow-md transition-shadow">
          <div className="w-12 h-12 bg-blue-50 text-blue-600 rounded-xl flex items-center justify-center mb-4">
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"></path></svg>
          </div>
          <h2 className="text-lg font-medium text-gray-900 mb-2">School Tenants</h2>
          <p className="text-sm text-gray-500 mb-6">Provision and manage new schools across the Eravaya platform.</p>
          <Link href="/dashboard/super-admin/schools/new" className="text-sm font-medium text-blue-600 hover:text-blue-700 flex items-center gap-1">
            Add New School 
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 5l7 7-7 7"></path></svg>
          </Link>
        </div>
        
        <div className="bg-white border border-gray-200 rounded-2xl p-6 shadow-sm hover:shadow-md transition-shadow">
          <div className="w-12 h-12 bg-purple-50 text-purple-600 rounded-xl flex items-center justify-center mb-4">
            <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z"></path></svg>
          </div>
          <h2 className="text-lg font-medium text-gray-900 mb-2">Global Users</h2>
          <p className="text-sm text-gray-500 mb-6">Manage all users, cross-school access, and platform administrators.</p>
          <button disabled className="text-sm font-medium text-gray-400 cursor-not-allowed">
            Coming Soon
          </button>
        </div>
      </div>
    </div>
  );
}
