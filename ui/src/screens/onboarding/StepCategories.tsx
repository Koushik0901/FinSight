interface Props { onNext: () => void; }
export default function StepCategories({ onNext }: Props) {
  return (
    <div>
      <h2>Confirm your categories</h2>
      <p>Filled in by Task 20.</p>
      <button onClick={onNext}>Use these →</button>
    </div>
  );
}
