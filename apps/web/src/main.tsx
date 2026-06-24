import { createRoot } from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App';
import './theme/tokens.css';
import './styles.css';

const queryClient = new QueryClient();

// No <StrictMode>: it double-invokes effects in dev, which would create/destroy
// the WebGPU surface twice per mount. The map canvas owns a GPU device, so it
// must mount exactly once.
createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={queryClient}>
    <App />
  </QueryClientProvider>,
);
