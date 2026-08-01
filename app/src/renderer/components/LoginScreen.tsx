import { useState } from "react";
import { KeyRound, Shield, LogIn, AlertTriangle } from "lucide-react";

interface LoginScreenProps {
  onLogin: (apiKey: string, serverUrl?: string) => Promise<void>;
  initialServerUrl?: string;
}

export function LoginScreen({ onLogin, initialServerUrl }: LoginScreenProps) {
  const [apiKey, setApiKey] = useState("");
  const [serverUrl, setServerUrl] = useState(initialServerUrl || "http://127.0.0.1:8090");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    const trimmed = apiKey.trim();
    if (!trimmed) {
      setError("Savant API key is required.");
      return;
    }

    setIsSubmitting(true);
    setError("");
    try {
      await onLogin(trimmed, serverUrl.trim());
    } catch (e: any) {
      setError(e?.message || "Login failed.");
      setIsSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 bg-[#080b12] flex items-center justify-center z-[1000] overflow-hidden">
      <div className="absolute inset-0 opacity-20 pointer-events-none">
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-[var(--cp-cyan)] blur-[120px]" />
        <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-[var(--cp-magenta)] blur-[120px]" />
      </div>

      <form
        onSubmit={handleSubmit}
        style={{
          background: "rgba(13, 18, 32, 0.92)",
          border: "1px solid var(--cp-border)",
          boxShadow: "0 0 30px rgba(0,229,255,0.12)",
        }}
        className="relative w-[90vw] max-w-md p-6"
      >
        <div className="flex items-center gap-3 mb-6">
          <div
            style={{ border: "1px solid var(--cp-cyan)", color: "var(--cp-cyan)" }}
            className="p-3"
          >
            <Shield size={24} />
          </div>
          <div>
            <h1
              style={{ color: "var(--cp-cyan)", fontFamily: "'Orbitron', sans-serif" }}
              className="text-base uppercase tracking-widest"
            >
              Savant Sanctum
            </h1>
            <p
              style={{ color: "var(--foreground)", fontFamily: "'Rajdhani', sans-serif" }}
              className="text-sm opacity-60"
            >
              Authenticate with your Savant API key
            </p>
          </div>
        </div>

        <label
          style={{ color: "var(--cp-cyan)", fontFamily: "'Share Tech Mono', monospace" }}
          className="block text-xs mb-2 opacity-70"
        >
          Server URL
        </label>
        <div
          style={{ background: "var(--cp-bg-3)", border: "1px solid var(--cp-border)" }}
          className="flex items-center gap-2 px-3 py-2 mb-4"
        >
          <input
            type="text"
            value={serverUrl}
            onChange={e => setServerUrl(e.target.value)}
            placeholder="http://127.0.0.1:8090"
            style={{
              background: "transparent",
              color: "var(--foreground)",
              fontFamily: "'Share Tech Mono', monospace",
              outline: "none",
              border: "none",
            }}
            className="flex-1 text-xs placeholder:opacity-30"
          />
        </div>

        <label
          style={{ color: "var(--cp-cyan)", fontFamily: "'Share Tech Mono', monospace" }}
          className="block text-xs mb-2 opacity-70"
        >
          Savant API Key
        </label>
        <div
          style={{ background: "var(--cp-bg-3)", border: "1px solid var(--cp-border)" }}
          className="flex items-center gap-2 px-3 py-2"
        >
          <KeyRound size={14} style={{ color: "var(--cp-cyan)", opacity: 0.7 }} />
          <input
            type="password"
            value={apiKey}
            onChange={e => setApiKey(e.target.value)}
            autoFocus={!serverUrl}
            placeholder="sk-..."
            style={{
              background: "transparent",
              color: "var(--foreground)",
              fontFamily: "'Share Tech Mono', monospace",
              outline: "none",
              border: "none",
            }}
            className="flex-1 text-xs placeholder:opacity-30"
          />
        </div>

        {error && (
          <div
            style={{ color: "var(--cp-magenta)", fontFamily: "'Share Tech Mono', monospace" }}
            className="flex items-center gap-2 mt-3 text-xs"
          >
            <AlertTriangle size={13} />
            <span>{error}</span>
          </div>
        )}

        <button
          type="submit"
          disabled={isSubmitting}
          style={{
            background: "var(--cp-cyan)",
            color: "var(--cp-bg-0)",
            fontFamily: "'Share Tech Mono', monospace",
          }}
          className="mt-6 w-full px-4 py-2 text-xs font-medium flex items-center justify-center gap-2 disabled:opacity-50"
        >
          <LogIn size={14} />
          {isSubmitting ? "AUTHENTICATING..." : "LOGIN"}
        </button>
      </form>
    </div>
  );
}
