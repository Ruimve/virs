import { WizardProvider } from '../context/WizardContext/WizardProvider';
import Layout from './Layout';

export const LayoutBox = () => {
  return (
    <WizardProvider>
      <Layout />
    </WizardProvider>
  );
};

export default LayoutBox;
