interface Props { onNext: () => void; }
export default function StepConnect({ onNext }: Props) {
  return (
    <div>
      <h2>Connect your money</h2>
      <p>This step gets filled in by Tasks 16 (manual entry) and 19 (CSV import).</p>
      <button onClick={onNext}>Skip for now →</button>
    </div>
  );
}
