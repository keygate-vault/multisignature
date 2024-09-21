import React, { useState, useEffect } from "react";
import AccountPageLayout from "../../AccountPageLayout";
import {
  Box,
  Typography,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Paper,
  Tabs,
  Tab,
  Button,
  CircularProgress,
  Chip,
} from "@mui/material";
import { VisibilityOff, Add } from "@mui/icons-material";
import AddTokenModal from "./AddTokenModal";
import {
  getTokens,
  createIcrcAccount,
  getIcrcAccount,
} from "../../../api/account";
import { useAccount } from "../../../contexts/AccountContext";
import { useInternetIdentity } from "../../../hooks/use-internet-identity";
import { TokenData } from "../../../types/assets";
import { Principal } from "@dfinity/principal";
import { extractTokenData } from "../../../util/token";
import { getTokenBalance, getTokenSymbol } from "../../../api/icrc";
import { base32ToBlob, hexToBytes } from "../../../util/conversion";

interface Asset {
  name: string;
  icon: string;
  balance: string;
  value: string;
  isIcrc: boolean;
}

interface TokenInfo {
  network: string;
  standard: string;
  principalId: string;
}

const Assets: React.FC = () => {
  const [tabValue, setTabValue] = useState(0);
  const [showTokens, setShowTokens] = useState(true);
  const [assets, setAssets] = useState<Asset[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const { vaultCanisterId, icpBalance } = useAccount();
  const { identity } = useInternetIdentity();

  const icrcTokenInfo = async (
    token: string,
    tokenInfo: TokenInfo
  ): Promise<Asset> => {
    const { principalId } = tokenInfo;
    let balance = "Unknown";

    console.log("Token", token);

    const subaccountResult = await getIcrcAccount(
      vaultCanisterId!,
      Principal.fromText(principalId),
      identity!
    );

    if (!("Ok" in subaccountResult)) {
      throw new Error(
        `Error fetching subaccount for string ${principalId} : ${JSON.stringify(
          subaccountResult.Err
        )}`
      );
    }

    console.log("Subaccount", subaccountResult.Ok);

    const subaccountBytes = hexToBytes(subaccountResult.Ok);

    console.log("Subaccount bytes", subaccountBytes);
    const rawBalance = await getTokenBalance(
      Principal.fromText(principalId),
      subaccountBytes
    );

    console.log("Raw balance", rawBalance);

    if (rawBalance === undefined) {
      throw new Error(`Error fetching balance for ${token}:`);
    }

    balance = rawBalance.toString();

    const symbol = await getTokenSymbol(Principal.fromText(principalId));

    return {
      name: `${symbol}`,
      icon: "🔸",
      balance,
      value: "N/A",
      isIcrc: true,
    };
  };

  const icTokenInfo = async (): Promise<Asset> => {
    const balance = icpBalance?.toString() || "Unknown";

    return {
      name: "ICP",
      icon: "🔹",
      balance,
      value: "N/A",
      isIcrc: false,
    };
  };

  const fetchAssetInfo = async (
    tokenPath: string,
    tokenInfo: TokenInfo
  ): Promise<Asset> => {
    const { network, standard } = tokenInfo;

    try {
      if (tokenPath.toLowerCase().includes("icp:native")) {
        return await icTokenInfo();
      } else {
        return await icrcTokenInfo(tokenPath, tokenInfo);
      }
    } catch (error) {
      console.error(`Error fetching asset info for ${tokenPath}:`, error);
      return {
        name: network === "ICP" ? "ICP" : `${standard.toUpperCase()}`,
        icon: network.toLowerCase() === "icp" ? "🔹" : "🔸",
        balance: "Error",
        value: "N/A",
        isIcrc: network !== "ICP",
      };
    }
  };

  const fetchAssets = async () => {
    if (vaultCanisterId && identity) {
      try {
        setLoading(true);
        const tokens = await getTokens(vaultCanisterId, identity);
        console.log("Tokens", tokens);
        const formattedAssets = await Promise.all(
          tokens.map(async (token) => {
            const tokenInfo = extractTokenData(token);
            return fetchAssetInfo(token, tokenInfo);
          })
        );
        setAssets(formattedAssets);
      } catch (error) {
        console.error("Error fetching tokens:", error);
      } finally {
        setLoading(false);
      }
    }
  };

  useEffect(() => {
    fetchAssets();
  }, [vaultCanisterId, identity, icpBalance]);

  const handleTabChange = (event: React.SyntheticEvent, newValue: number) => {
    setTabValue(newValue);
  };

  const handleTokenVisibility = () => {
    setShowTokens(!showTokens);
  };

  const handleOpenModal = () => {
    setModalOpen(true);
  };

  const handleCloseModal = () => {
    setModalOpen(false);
  };

  const handleAddToken = async (tokenData: TokenData) => {
    if (vaultCanisterId && identity) {
      try {
        const icrcAccount = await createIcrcAccount(
          vaultCanisterId,
          Principal.fromText(tokenData.address),
          identity
        );
        await fetchAssets();
      } catch (error) {
        console.error("Error creating ICRC account:", error);
      }
    }
  };

  if (loading) {
    return (
      <AccountPageLayout>
        <Box
          sx={{
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            height: "100%",
          }}
        >
          <CircularProgress />
        </Box>
      </AccountPageLayout>
    );
  }

  return (
    <AccountPageLayout>
      <Box sx={{ width: "100%", color: "text.primary" }}>
        <Typography variant="h4" sx={{ mb: 3 }}>
          Assets
        </Typography>

        <Box sx={{ borderBottom: 1, borderColor: "divider", mb: 2 }}>
          <Tabs value={tabValue} onChange={handleTabChange} textColor="inherit">
            <Tab label="Tokens" />
            <Tab label="NFTs" />
          </Tabs>
        </Box>

        <Box sx={{ display: "flex", justifyContent: "space-between", mb: 2 }}>
          <Button
            variant="outlined"
            startIcon={<VisibilityOff />}
            onClick={handleTokenVisibility}
          >
            {showTokens ? "Hide" : "Show"} token amounts
          </Button>
          <Button
            variant="contained"
            startIcon={<Add />}
            onClick={handleOpenModal}
          >
            Add Token
          </Button>
        </Box>

        <TableContainer
          component={Paper}
          sx={{
            borderRadius: 0,
            "& .MuiPaper-root": { borderRadius: 0 },
            marginTop: 4,
          }}
        >
          <Table>
            <TableHead>
              <TableRow>
                <TableCell>Asset</TableCell>
                <TableCell align="right">Balance</TableCell>
                <TableCell align="right">Value</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {assets.map((asset) => (
                <TableRow
                  key={asset.name}
                  sx={{
                    "&:last-child td, &:last-child th": {
                      border: 0,
                    },
                  }}
                >
                  <TableCell component="th" scope="row">
                    <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
                      <Chip
                        label={asset.isIcrc ? "ICRC" : "Native"}
                        size="small"
                        color={asset.isIcrc ? "primary" : "secondary"}
                      />
                      {asset.name}
                    </Box>
                  </TableCell>
                  <TableCell align="right">
                    {showTokens ? asset.balance : "****"}
                  </TableCell>
                  <TableCell align="right">
                    {showTokens ? asset.value : "****"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Box>
      <AddTokenModal
        open={modalOpen}
        handleClose={handleCloseModal}
        handleAddToken={handleAddToken}
      />
    </AccountPageLayout>
  );
};

export default Assets;
