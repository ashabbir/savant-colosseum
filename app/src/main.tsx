import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './renderer/App';
import './renderer/styles/tailwind.css';
import './renderer/styles/theme.css';
import './renderer/styles.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
