import React, { useState } from 'react';
import { STAFF_ROLES, StaffRole, CsvColumn } from '../rolesConfig';

export function AddUser() {
  const [selectedRole, setSelectedRole] = useState<StaffRole | ''>('');
  const [formData, setFormData] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');

  const selectedConfig = STAFF_ROLES.find(r => r.value === selectedRole);

  const handleInputChange = (header: string, value: string) => {
    setFormData(prev => ({ ...prev, [header]: value }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedConfig) return;

    setLoading(true);
    setMessage('');

    try {
      const token = localStorage.getItem('token');
      // For now, this points to single staff endpoint (assuming we create one, or you can route it)
      const res = await fetch(`/api/v1/users/staff/single?role=${selectedRole}`, {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(formData)
      });

      const data = await res.json();
      if (res.ok) {
        setMessage('User added successfully!');
        setFormData({});
      } else {
        setMessage(data.error || 'Failed to add user');
      }
    } catch (err) {
      setMessage('Network error occurred.');
    } finally {
      setLoading(false);
    }
  };

  const renderInput = (col: CsvColumn) => {
    let inputType = 'text';
    if (col.type === 'Email') inputType = 'email';
    if (col.type === 'Phone') inputType = 'tel';
    if (col.type === 'Date') inputType = 'date';

    return (
      <div key={col.header} className="mb-4">
        <label className="block text-sm font-medium text-gray-700 mb-1">
          {col.header} {col.required && <span className="text-red-500">*</span>}
        </label>
        <input
          type={inputType}
          required={col.required}
          className="w-full p-2 border border-gray-300 rounded-md focus:ring-blue-500 focus:border-blue-500"
          value={formData[col.header] || ''}
          onChange={(e) => handleInputChange(col.header, e.target.value)}
        />
      </div>
    );
  };

  return (
    <div className="p-6 max-w-2xl mx-auto bg-white rounded-lg shadow-sm">
      <h2 className="text-2xl font-bold mb-6 text-gray-800">Add Staff Member</h2>
      
      <div className="mb-6">
        <label className="block text-sm font-medium text-gray-700 mb-2">Select Staff Role</label>
        <select 
          className="w-full p-3 border border-gray-300 rounded-md focus:ring-blue-500 focus:border-blue-500 bg-gray-50"
          value={selectedRole}
          onChange={(e) => {
            setSelectedRole(e.target.value as StaffRole);
            setFormData({}); // Reset form when role changes
            setMessage('');
          }}
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
        <form onSubmit={handleSubmit} className="border-t pt-6 border-gray-100">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-4">
            {selectedConfig.schema.map(renderInput)}
          </div>

          <div className="mt-6">
            <button 
              type="submit"
              disabled={loading}
              className="w-full py-3 bg-blue-600 text-white font-medium rounded-md hover:bg-blue-700 disabled:bg-blue-300 transition"
            >
              {loading ? 'Adding User...' : `Add ${selectedConfig.label}`}
            </button>
          </div>

          {message && (
            <div className={`mt-4 p-3 rounded-md ${message.includes('success') ? 'bg-green-50 text-green-800' : 'bg-red-50 text-red-800'}`}>
              {message}
            </div>
          )}
        </form>
      )}
    </div>
  );
}
