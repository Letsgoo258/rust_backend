"use client";

import { useEffect } from 'react';
import {
  initializeFaro,
  getWebInstrumentations,
  ReactIntegration,
} from '@grafana/faro-react';
import { TracingInstrumentation } from '@grafana/faro-web-tracing';

export default function FaroProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    const faroUrl = process.env.NEXT_PUBLIC_FARO_URL;
    
    if (faroUrl && typeof window !== 'undefined') {
      initializeFaro({
        url: faroUrl,
        app: {
          name: 'eravaya-frontend',
          version: '0.1.0',
          environment: process.env.NODE_ENV,
        },
        instrumentations: [
          ...getWebInstrumentations(),
          new TracingInstrumentation(),
          new ReactIntegration(),
        ],
      });
    }
  }, []);

  return <>{children}</>;
}
