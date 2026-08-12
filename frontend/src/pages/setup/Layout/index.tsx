import { Outlet } from 'react-router-dom';
import { Layout } from '@/layout';
import { WizardProvider } from '../context/WizardContext/WizardProvider';
import Header from './Header';

export const LayoutBox = () => {
  return (
    <WizardProvider>
      <Layout header={<Header />} sidebar={false}>
        <Outlet />
      </Layout>
    </WizardProvider>
  );
};

export default LayoutBox;
