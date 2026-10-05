import { motion } from 'motion/react';
import { ReactNode } from 'react';

interface TabWrapperProps {
  children: ReactNode;
  id: string;
  className?: string;
}

export function TabWrapper({ children, id, className = '' }: TabWrapperProps) {
  return (
    <motion.div
      key={id}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.25, ease: 'easeInOut' }}
      className={`bg-background overflow-hidden flex flex-col transition-colors duration-300 ${className}`}
    >
      {children}
    </motion.div>
  );
}
