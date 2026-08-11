import { Outlet } from 'react-router-dom';
import { AppShell } from '@/layout/AppShell';
import { WizardProvider } from '../context/WizardContext/WizardProvider';
import Header from './Header';

export const LayoutBox = () => {
  return (
    <WizardProvider>
      <AppShell header={<Header />} sidebar={false}>
        <Outlet />
      </AppShell>
    </WizardProvider>
  );
};

export default LayoutBox;
