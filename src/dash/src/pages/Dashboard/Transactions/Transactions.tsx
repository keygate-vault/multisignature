import React, { useState, useEffect } from "react";
import {
  Box,
  Typography,
  Tabs,
  Tab,
  List,
  ListItem,
  ListItemText,
  ListItemIcon,
  Divider,
  Chip,
  Button,
  Paper,
  Badge,
} from "@mui/material";
import {
  Send as SendIcon,
  SwapHoriz as SwapIcon,
  AccountBalanceWallet as WalletIcon,
  InfoOutlined as InfoIcon,
  CheckCircle as ApprovedIcon,
  Cancel as RejectedIcon,
} from "@mui/icons-material";
import AccountPageLayout from "../../VaultPageLayout";
import {
  TransactionRequest,
  IntentStatus,
  Transaction,
  ProposedTransaction,
} from "../../../../../declarations/account/account.did";
import { useInternetIdentity } from "../../../hooks/use-internet-identity";
import { useVaultDetail } from "../../../contexts/VaultDetailContext";
import { TOKEN_URN_TO_SYMBOL } from "../../../util/constants";
import {
  getTransactions,
  getProposedTransactions,
  getThreshold,
} from "../../../api/account";
import { E8sToIcp } from "../../../util/units";
import { Principal } from "@dfinity/principal";
import { executeTransaction } from "../../../api/account";

const Transactions: React.FC = () => {
  const [executedTransactions, setExecutedTransactions] = useState<
    Transaction[]
  >([]);
  const [proposedTransactions, setProposedTransactions] = useState<
    ProposedTransaction[]
  >([]);
  const [threshold, setThreshold] = useState<bigint>(BigInt(0));
  const [isLoading, setIsLoading] = useState(true);
  const [tabValue, setTabValue] = useState(0);
  const { vaultCanisterId, nativeAccountId } = useVaultDetail();
  const { identity } = useInternetIdentity();

  useEffect(() => {
    const fetchData = async () => {
      if (vaultCanisterId && nativeAccountId) {
        try {
          let [executed, proposed, thresholdValue] = await Promise.all([
            getTransactions(vaultCanisterId, identity!),
            getProposedTransactions(vaultCanisterId, identity!),
            getThreshold(vaultCanisterId, identity!),
          ]);

          executed = executed.reverse();

          setExecutedTransactions(executed || []);
          setProposedTransactions(proposed || []);
          setThreshold(thresholdValue);

          // Set initial tab value based on threshold
          if (thresholdValue <= BigInt(1)) {
            setTabValue(1);
          }
          setIsLoading(false);
        } catch (error) {
          console.error("Error fetching data:", error);
        } finally {
        }
      }
    };

    fetchData();
  }, [nativeAccountId, vaultCanisterId]);

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    if (threshold > BigInt(1) || newValue === 1) {
      setTabValue(newValue);
    }
  };

  const renderIntentIcon = (type: { [key: string]: null }) => {
    if ("Transfer" in type) {
      return <SendIcon sx={{ color: "white" }} />;
    } else if ("Swap" in type) {
      return <SwapIcon sx={{ color: "white" }} />;
    } else {
      return <WalletIcon sx={{ color: "white" }} />;
    }
  };

  const formatAmount = (amount: number, token: string) => {
    const formattedAmount = (amount);
    return `${formattedAmount.toFixed(4).toLocaleString()} ${TOKEN_URN_TO_SYMBOL[token]
      }`;
  };

  const getIntentStatus = (status: IntentStatus): string => {
    if ("Pending" in status) return "Pending";
    if ("InProgress" in status) return "InProgress";
    if ("Completed" in status) return "Completed";
    if ("Rejected" in status) return "Rejected";
    if ("Failed" in status) return "Failed";
    return "Unknown";
  };

  const renderSignersInfo = (signers: Principal[], rejections: Principal[]) => {
    return (
      <Box sx={{ mt: 1 }}>
        <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap", mb: 1 }}>
          {signers.map((signer, index) => (
            <Chip
              key={`signer-${index}`}
              icon={<ApprovedIcon sx={{ fontSize: 16 }} />}
              label={signer.toString().slice(0, 8) + "..."}
              size="small"
              sx={{
                backgroundColor: "rgba(76, 175, 80, 0.1)",
                borderColor: "success.main",
                color: "success.main",
              }}
              variant="outlined"
            />
          ))}
        </Box>
        <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>
          {rejections.map((rejector, index) => (
            <Chip
              key={`rejector-${index}`}
              icon={<RejectedIcon sx={{ fontSize: 16 }} />}
              label={rejector.toString().slice(0, 8) + "..."}
              size="small"
              sx={{
                backgroundColor: "rgba(244, 67, 54, 0.1)",
                borderColor: "error.main",
                color: "error.main",
              }}
              variant="outlined"
            />
          ))}
        </Box>
      </Box>
    );
  };

  const renderExecutedTransactions = () => {
    if (executedTransactions.length === 0) {
      return (
        <Paper sx={{ p: 3 }}>
          <Box
            sx={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 2,
            }}
          >
            <InfoIcon sx={{ fontSize: 48 }} />
            <Typography variant="h6">No executed transactions found</Typography>
          </Box>
        </Paper>
      );
    }

    return (
      <List>
        {executedTransactions.map((transaction, index) => (
          <React.Fragment key={index.toString()}>
            {index > 0 && <Divider component="li" />}
            <ListItem alignItems="flex-start" sx={{ py: 2 }}>
              <ListItemIcon>
                {renderIntentIcon(transaction.transaction_type)}
              </ListItemIcon>
              <ListItemText
                primary={
                  <Box
                    sx={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                    }}
                  >
                    <Typography variant="body1" sx={{ color: "white" }}>
                      {Object.keys(transaction.transaction_type)[0]}
                    </Typography>
                    <Typography component="span" sx={{ color: "white" }}>
                      {formatAmount(transaction.amount, transaction.token)}
                    </Typography>
                  </Box>
                }
                secondary={
                  <React.Fragment>
                    <Typography variant="body2">
                      To: {transaction.to}
                    </Typography>
                    <Box
                      sx={{
                        display: "flex",
                        justifyContent: "space-between",
                        mt: 1,
                      }}
                    >
                      <Chip
                        label={Object.keys(transaction.network)[0]}
                        size="small"
                      />
                      <Chip
                        label={getIntentStatus(transaction.status)}
                        size="small"
                      />
                    </Box>
                  </React.Fragment>
                }
              />
            </ListItem>
          </React.Fragment>
        ))}
      </List>
    );
  };

  const renderProposedTransactions = () => {
    if (proposedTransactions.length === 0) {
      return (
        <Paper sx={{ p: 3 }}>
          <Box
            sx={{
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 2,
            }}
          >
            <InfoIcon sx={{ fontSize: 48 }} />
            <Typography variant="h6">No proposed transactions found</Typography>
          </Box>
        </Paper>
      );
    }

    return (
      <>
        <List>
          {proposedTransactions.map((proposal, index) => (
            <React.Fragment key={index.toString()}>
              {index > 0 && <Divider component="li" />}
              <ListItem alignItems="flex-start" sx={{ py: 2 }}>
                <ListItemIcon>
                  {renderIntentIcon(proposal.transaction_type)}
                </ListItemIcon>
                <ListItemText
                  primary={
                    <Box
                      sx={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <Typography variant="body1" sx={{ color: "white" }}>
                        {Object.keys(proposal.transaction_type)[0]}
                      </Typography>
                      <Typography component="span" sx={{ color: "white" }}>
                        {formatAmount(proposal.amount, proposal.token)}
                      </Typography>
                    </Box>

                  }
                  secondary={
                    <React.Fragment>
                      <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
                        <Box>
                          <Typography variant="body2">To: {proposal.to}</Typography>
                          <Typography variant="body2" sx={{ mt: 1 }}>
                            Proposal ID: {proposal.id.toString()}
                          </Typography>
                        </Box>
                        <Box
                          sx={{
                            mt: 1,
                          }}>
                          {proposal.signers.length >= Number(threshold) && (
                            <Button
                              variant="contained"
                              color="primary"
                              onClick={() => executeTransaction(vaultCanisterId, proposal.id, identity!)}
                            >
                              Execute
                            </Button>
                          )}
                        </Box>
                      </Box>
                      {renderSignersInfo(proposal.signers, proposal.rejections)}
                      <Box
                        sx={{
                          display: "flex",
                          justifyContent: "space-between",
                          mt: 1,
                        }}
                      >
                        <Chip
                          label={Object.keys(proposal.network)[0]}
                          size="small"
                        />
                        <Chip
                          label={`${proposal.signers.length
                            }/${threshold.toString()} signatures`}
                          size="small"
                          color={
                            proposal.signers.length >= Number(threshold)
                              ? "success"
                              : "default"
                          }
                        />
                      </Box>
                    </React.Fragment>
                  }
                />
              </ListItem>
            </React.Fragment>
          ))}
        </List>
      </>
    );
  };

  const renderContent = () => {
    if (isLoading) {
      return <Typography>Loading...</Typography>;
    }

    return tabValue === 0
      ? renderProposedTransactions()
      : renderExecutedTransactions();
  };

  return (
    <AccountPageLayout>
      <Box>
        <Typography variant="h4" gutterBottom sx={{ color: "white" }}>
          Transactions
        </Typography>
        <Box
          sx={{
            borderBottom: 1,
            borderColor: "rgba(255, 255, 255, 0.12)",
            mb: 2,
          }}
        >
          <Tabs
            value={tabValue}
            onChange={handleTabChange}
            textColor="inherit"
            sx={{
              "& .MuiTab-root": { color: "white" },
              "& .Mui-selected": { color: "primary.main" },
            }}
          >
            <Tab
              label="Proposed"
              disabled={threshold <= BigInt(1)}
              sx={{
                opacity: threshold <= BigInt(1) ? 0.5 : 1,
              }}
            />
            <Tab label="Executed" />
          </Tabs>
        </Box>
        {renderContent()}
      </Box>
    </AccountPageLayout>
  );
};

export default Transactions;
