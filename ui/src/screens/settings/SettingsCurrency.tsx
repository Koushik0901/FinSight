import { useDefaultCurrency, useSetCurrency } from "../../api/hooks/settings";
import { Section } from "./Section";

const CURRENCIES = ["USD", "EUR", "GBP", "CAD", "AUD", "JPY"];

export default function SettingsCurrency() {
  const setCurrencyMutation = useSetCurrency();
  const { data: currentCurrency = "USD" } = useDefaultCurrency();
  return (
    <Section id="currency" title="Currency" description="Used for all money formatting in the app.">
      <div className="s-row">
        <div>
          <div className="label" id="settings-currency-label">
            Currency
          </div>
          <div className="desc">Used for all money formatting in the app.</div>
        </div>
        <div>
          <select
            className="control"
            aria-labelledby="settings-currency-label"
            value={currentCurrency}
            onChange={(e) => setCurrencyMutation.mutate(e.target.value)}
            style={{ maxWidth: 140 }}
          >
            {CURRENCIES.map((currency) => (
              <option key={currency} value={currency}>
                {currency}
              </option>
            ))}
          </select>
        </div>
        <div />
      </div>
    </Section>
  );
}
