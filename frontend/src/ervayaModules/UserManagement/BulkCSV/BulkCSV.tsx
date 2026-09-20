import React, { useState } from 'react';
import { STAFF_ROLES, StaffRole } from '../rolesConfig';

export function BulkCSV() {
  const [selectedRole, setSelectedRole] = useState<StaffRole | ''>('');
  const [file, setFile] = useState<File | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');
  const [errorCsv, setErrorCsv] = useState<string | null>(null);

  const selectedConfig = STAFF_ROLES.find(r => r.value === selectedRole);

  const handleDownloadTemplate = () => {
    if (!selectedConfig) return;
    const headers = selectedConfig.schema.map(c => c.header).join(',');
    const blob = new Blob([headers], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${selectedConfig.value.toLowerCase()}_template.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleUpload = async () => {
    if (!file || !selectedRole) return;
    setLoading(true);
    setMessage('');
    setErrorCsv(null);

    const formData = new FormData();
    formData.append('file', file);

    try {
      const token = localStorage.getItem('token');
      const res = await fetch(`/api/v1/users/staff/bulk?role=${selectedRole}`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`
        },
        body: formData
      });

      const data = await res.json();
      if (res.ok) {
        setMessage(data.message || 'Upload successful!');
        if (data.errors_base64) {
          setErrorCsv(data.errors_base64);
        }
      } else {
        setMessage(data.error || 'Upload failed');
      }
    } catch (err) {
      setMessage('Network error occurred during upload.');
    } finally {
      setLoading(false);
    }
  };

  const handleDownloadErrors = () => {
    if (!errorCsv) return;
    const decoded = atob(errorCsv);
    const blob = new Blob([decoded], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `failed_rows.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="p-6 max-w-4xl mx-auto bg-white rounded-lg shadow-sm">
      <h2 className="text-2xl font-bold mb-6 text-gray-800">Bulk Onboard Staff</h2>
      
      <div className="mb-6">
        <label className="block text-sm font-medium text-gray-700 mb-2">1. Select Staff Role & Department</label>
        <select 
          className="w-full p-3 border border-gray-300 rounded-md focus:ring-blue-500 focus:border-blue-500"
          value={selectedRole}
          onChange={(e) => setSelectedRole(e.target.value as StaffRole)}
        >
          <option value="">-- Choose a Role --</option>
          {STAFF_ROLES.map(role => (
            <option key={role.value} value={role.value}>
              {role.label} - {role.category}
            </option>
          ))}
        </select>
      </div>

      {selectedConfig && (
        <div className="mb-6 p-4 bg-blue-50 rounded-md border border-blue-100">
          <h3 className="font-semibold text-blue-800">{selectedConfig.label}</h3>
          <p className="text-sm text-blue-600 mb-3">{selectedConfig.description}</p>
          
          <h4 className="text-sm font-medium text-gray-700 mb-2">Required CSV Columns:</h4>
          <div className="flex flex-wrap gap-2 mb-4">
            {selectedConfig.schema.map(col => (
              <span key={col.header} className="px-2 py-1 bg-white border border-gray-200 rounded text-xs text-gray-600 shadow-sm">
                {col.header} {col.required ? <span className="text-red-500">*</span> : ''}
              </span>
            ))}
          </div>
          
          <button 
            onClick={handleDownloadTemplate}
            className="text-sm px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition"
          >
            Download CSV Template
          </button>
        </div>
      )}

      <div className={`mb-6 p-6 border-2 border-dashed rounded-md text-center transition-colors ${file ? 'border-green-400 bg-green-50' : 'border-gray-300 hover:border-gray-400'}`}>
        <label className="cursor-pointer block">
          <span className="text-gray-600 block mb-2">{file ? file.name : '2. Drag & Drop CSV here or click to browse'}</span>
          <input 
            type="file" 
            accept=".csv" 
            className="hidden" 
            onChange={(e) => setFile(e.target.files?.[0] || null)}
          />
        </label>
      </div>

      <button 
        onClick={handleUpload}
        disabled={!file || !selectedRole || loading}
        className="w-full py-3 bg-gray-800 text-white font-medium rounded-md hover:bg-gray-900 disabled:bg-gray-300 transition"
      >
        {loading ? 'Uploading & Validating...' : '3. Process Bulk Upload'}
      </button>

      {message && (
        <div className={`mt-4 p-4 rounded-md ${errorCsv ? 'bg-orange-50 border border-orange-200 text-orange-800' : 'bg-green-50 border border-green-200 text-green-800'}`}>
          <p>{message}</p>
          {errorCsv && (
            <button onClick={handleDownloadErrors} className="mt-2 text-sm text-orange-700 underline font-medium">
              Download Failed Rows (Errors Attached)
            </button>
          )}
        </div>
      )}
    </div>
  );
}
