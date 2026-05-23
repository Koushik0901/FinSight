interface Props { onDone: () => void; }
export default function StepAgent({ onDone }: Props) {
  return (
    <div>
      <h2>Set up the agent</h2>
      <p>Filled in by Task 21.</p>
      <button onClick={onDone}>Finish →</button>
    </div>
  );
}
