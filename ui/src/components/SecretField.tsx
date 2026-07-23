import { useId, useState, type ChangeEventHandler, type ReactNode } from 'react';

type SecretFieldProps = {
  id: string;
  label: string;
  name: string;
  value: string;
  onChange: ChangeEventHandler<HTMLInputElement>;
  hintId: string;
  hint: ReactNode;
  placeholder?: string;
  showLabel?: string;
  hideLabel?: string;
};

function EyeIcon({ crossed }: { crossed?: boolean }) {
  return (
    <svg
      aria-hidden="true"
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
      <circle cx="12" cy="12" r="3" />
      {crossed ? <path d="M4 4l16 16" /> : null}
    </svg>
  );
}

export default function SecretField({
  id,
  label,
  name,
  value,
  onChange,
  hintId,
  hint,
  placeholder = '••••••••',
  showLabel = 'Show value',
  hideLabel = 'Hide value',
}: SecretFieldProps) {
  const [visible, setVisible] = useState(false);
  const toggleId = useId();

  return (
    <>
      <label htmlFor={id}>{label}</label>
      <div className="secret-field">
        <input
          id={id}
          name={name}
          type={visible ? 'text' : 'password'}
          autoComplete="off"
          spellCheck={false}
          placeholder={placeholder}
          aria-describedby={hintId}
          value={value}
          onChange={onChange}
        />
        <button
          id={toggleId}
          type="button"
          className="secret-field__toggle"
          aria-label={visible ? hideLabel : showLabel}
          aria-pressed={visible}
          aria-controls={id}
          onClick={() => setVisible((v) => !v)}
        >
          <EyeIcon crossed={visible} />
        </button>
      </div>
      <p id={hintId} className="field-hint">
        {hint}
      </p>
    </>
  );
}
